{
  description = "Arto - the Art of Reading Markdown";

  nixConfig = {
    extra-substituters = [ "https://arto.cachix.org" ];
    extra-trusted-public-keys = [ "arto.cachix.org-1:yaH0JQomRJTosIcTh2xZPKBEny41D7h6QUePYQzWYqc=" ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
    }:
    let
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];
      eachSystem = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = eachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          inherit (pkgs) lib;
          craneLib = crane.mkLib pkgs;

          # Package metadata - version resolved from VERSION file (CI) or git rev (local)
          cargoToml = builtins.fromTOML (builtins.readFile ./desktop/Cargo.toml);
          versionFile = ./desktop/VERSION;
          artoVersion =
            if builtins.pathExists versionFile
            then builtins.replaceStrings [ "\n" ] [ "" ] (builtins.readFile versionFile)
            else "${cargoToml.package.version}-${self.dirtyShortRev or self.shortRev or "unknown"}";
          packageMeta = {
            pname = cargoToml.package.name;
            version = artoVersion;
          };

          # Platform detection
          isDarwin = pkgs.stdenv.hostPlatform.isDarwin;
          isLinux = pkgs.stdenv.hostPlatform.isLinux;

          # App bundle paths (used in build and apps)
          appBundleName = "Arto.app";
          appExecutableName = "arto"; # lowercase executable name
          dxBundlePath =
            if isDarwin then
              "target/dx/${packageMeta.pname}/bundle/macos/macos"
            # dx build (not bundle) outputs here; bundle fails in Nix sandbox
            # due to permission errors in the .deb/.AppImage packagers.
            else
              "target/dx/${packageMeta.pname}/release/linux/app";

          renderer-assets = pkgs.stdenvNoCC.mkDerivation (finalAttrs: {
            pname = "${packageMeta.pname}-renderer-assets";
            inherit (packageMeta) version;
            src = ./renderer;

            nativeBuildInputs = [
              pkgs.nodejs-slim
              pkgs.pnpm_10
              pkgs.pnpmConfigHook
            ];

            pnpmDeps = pkgs.fetchPnpmDeps {
              inherit (finalAttrs) pname version src;
              pnpm = pkgs.pnpm_10;
              # To update this hash when renderer dependencies change:
              # 1. Change hash to: lib.fakeHash or ""
              # 2. Run: nix build .#renderer-assets
              # 3. Copy the expected hash from error message
              # 4. Update hash value below
              hash = "sha256-p2Ay6zKIXnG5BoqBJVpWJPBKzEkQsU97hvlujIas63U=";
              fetcherVersion = 3;
            };

            buildPhase = ''
              runHook preBuild
              # Override output directory for Nix build
              export VITE_OUT_DIR=$out
              pnpm run build
              runHook postBuild
            '';

            installPhase = ''
              runHook preInstall
              # Vite outputs directly to $out when VITE_OUT_DIR is set
              runHook postInstall
            '';
          });

          commonArgs = {
            src = lib.fileset.toSource rec {
              root = ./desktop;
              fileset = lib.fileset.unions [
                (craneLib.fileset.commonCargoSources root)
                # The shared `arto-markdown` crate is a path dependency nested
                # under desktop; include its sources for the sandboxed build.
                (craneLib.fileset.commonCargoSources (root + /markdown))
                (root + /assets)
                (root + /Dioxus.toml)
                (root + /src/keybindings/presets)
                (lib.fileset.maybeMissing (root + /VERSION))
              ];
            };
            strictDeps = true;
            # Pass version to build.rs via environment variable
            ARTO_BUILD_VERSION = artoVersion;
            nativeBuildInputs = lib.optionals isLinux [
              pkgs.pkg-config
            ];
            buildInputs =
              lib.optionals isDarwin [
                pkgs.libiconv
              ]
              ++ lib.optionals isLinux [
                pkgs.webkitgtk_4_1
                pkgs.gtk3
                pkgs.libsoup_3
                pkgs.glib
                pkgs.openssl
                pkgs.xdotool
              ];
          };

          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          # Build-time wrappers for macOS commands
          # See scripts/codesign-wrapper.sh and scripts/xattr-wrapper.sh for details
          codesignWrapper = pkgs.writeShellScriptBin "codesign" (
            builtins.replaceStrings
              [ "@CODESIGN_BIN@" ]
              [ "${pkgs.darwin.sigtool}/bin/codesign" ]
              (builtins.readFile ./scripts/codesign-wrapper.sh)
          );

          xattrWrapper = pkgs.writeShellScriptBin "xattr" (
            builtins.readFile ./scripts/xattr-wrapper.sh
          );

          # NOTE: The macOS Quick Look preview extension (Swift shim + Rust
          # `arto_ql` static library) is assembled and embedded by the
          # `dx bundle` path in `desktop/justfile` (`_embed-quicklook`), which
          # runs on the macOS CI runner with the full Command Line Tools. The
          # Nix build intentionally does not build or embed it.

          arto = craneLib.buildPackage (
            commonArgs
            // {
              inherit (packageMeta) pname version;
              inherit cargoArtifacts;

              # Disable: some tests depend on macOS display APIs (NSScreen) which
              # are unavailable in the Nix sandbox. Run tests via `cargo test` instead.
              doCheck = false;

              nativeBuildInputs =
                # Wrappers must come first to override system commands in PATH
                lib.optionals isDarwin [
                  codesignWrapper
                  xattrWrapper
                ]
                ++ [
                  pkgs.dioxus-cli
                ]
                ++ lib.optionals isDarwin [
                  pkgs.darwin.autoSignDarwinBinariesHook
                ]
                ++ lib.optionals isLinux [
                  pkgs.pkg-config
                  pkgs.wrapGAppsHook3
                ];

              postPatch = ''
                mkdir -p assets/dist
                cp -r ${renderer-assets}/* assets/dist/

                # Dioxus.toml references "../extras/mac/Arto.icns" and "../LICENSE"
                # Copy them from project root to satisfy relative path requirements
                cp -r ${./extras} ../extras
                cp ${./LICENSE} ../LICENSE
              '';

              # Use buildPhaseCargoCommand instead of cargoBuildCommand because crane's
              # additional build argument `--message-format` cannot be passed to dioxus-cli properly.
              # https://crane.dev/API.html#cranelibbuildpackage
              buildPhaseCargoCommand =
                if isDarwin then
                  ''
                    dx bundle --release --platform desktop --package-types macos
                  ''
                else
                  ''
                    dx build --release --platform desktop
                  '';

              # The build output is a platform-specific bundle, and crane cannot infer the
              # install destination, so we manually install without capturing cargoBuildLog
              # in buildPhase.
              # https://crane.dev/API.html#cranelibinstallfromcargobuildloghook
              doNotPostBuildInstallCargoBinaries = true;

              installPhaseCommand = lib.optionalString isDarwin ''
                # Find .app bundle (path may change with dioxus-cli versions)
                app_path="${dxBundlePath}/${appBundleName}"

                if [[ ! -d "$app_path" ]]; then
                  echo "Error: Expected .app bundle not found at $app_path"
                  echo "Searching for ${appBundleName} in target/dx..."
                  find target/dx -name "${appBundleName}" -type d || true
                  exit 1
                fi

                mkdir -p $out/Applications
                cp -r "$app_path" $out/Applications/

                # Create symlink for CLI usage (enables `arto` command in PATH)
                mkdir -p $out/bin
                ln -s "$out/Applications/${appBundleName}/Contents/MacOS/${appExecutableName}" "$out/bin/${appExecutableName}"
              ''
              + lib.optionalString isLinux ''
                app_dir="${dxBundlePath}"

                if [[ ! -d "$app_dir" ]]; then
                  echo "Error: Expected build output not found at $app_dir"
                  echo "Searching for build output in target/dx..."
                  find target/dx -type d 2>/dev/null || true
                  exit 1
                fi

                # Install the entire app directory (binary + assets) since
                # Dioxus asset!() macro resolves paths relative to the binary.
                mkdir -p $out/lib/${appExecutableName}
                cp -r "$app_dir"/. $out/lib/${appExecutableName}/

                mkdir -p $out/bin
                ln -s $out/lib/${appExecutableName}/${appExecutableName} $out/bin/${appExecutableName}
              '';
            }
          );
        in
        {
          default = self.packages.${system}.arto;
          inherit arto renderer-assets;
        }
      );

      apps = eachSystem (
        system:
        let
          # Access packageMeta from packages let-binding
          inherit (self.packages.${system}) arto;
          pkgs = nixpkgs.legacyPackages.${system};
          appBundleName = "Arto.app";
          appExecutableName = "arto";
        in
        {
          default = {
            type = "app";
            program =
              if pkgs.stdenv.hostPlatform.isDarwin then
                "${arto}/Applications/${appBundleName}/Contents/MacOS/${appExecutableName}"
              else
                "${arto}/bin/${appExecutableName}";
          };
        }
      );

      devShells = eachSystem (system: {
        default =
          let
            pkgs = nixpkgs.legacyPackages.${system};
            craneLib = crane.mkLib pkgs;
          in
          craneLib.devShell {
            inputsFrom = with self.packages.${system}; [ renderer-assets ];
            packages = [
              # Rust tools (craneLib.devShell provides: cargo, rustc, rustfmt, clippy, cargo-nextest)
              # We only add additional tools not included by default:
              pkgs.rust-analyzer # IDE support

              # Dioxus desktop development
              pkgs.dioxus-cli

              # TypeScript/renderer development (renderer/)
              pkgs.nodejs-slim
              pkgs.pnpm_10

              # Build automation
              pkgs.just
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
              pkgs.libiconv
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
              pkgs.pkg-config
              pkgs.webkitgtk_4_1
              pkgs.gtk3
              pkgs.libsoup_3
              pkgs.glib
              pkgs.openssl
              pkgs.xdotool
            ];

            # Workaround: Nix sets DEVELOPER_DIR to its apple-sdk, which breaks `just build` dmg creation.
            # https://github.com/NixOS/nixpkgs/issues/355486
            # RUST_SRC_PATH exposes Rust standard library sources for rust-analyzer and similar tools.
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";

            shellHook = ''
              unset DEVELOPER_DIR
              echo "🦀 Rust development environment"
              echo "  - cargo: $(cargo --version)"
              echo "  - rustc: $(rustc --version)"
              echo "  - dioxus-cli: $(dx --version)"
              echo ""
              echo "📦 TypeScript development environment"
              echo "  - node: $(node --version)"
              echo "  - pnpm: $(pnpm --version)"
              echo ""
              echo "🔧 Build tools"
              echo "  - just: $(just --version)"
            '';
          };
      });
    };
}
