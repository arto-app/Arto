{
  description = "Arto - the Art of Reading Markdown";

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
        "x86_64-darwin"
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

          # Package metadata - single source of truth for version and paths
          packageMeta = {
            pname = "arto";
            version = "0.0.0";
          };

          # Platform detection
          isDarwin = pkgs.stdenv.hostPlatform.isDarwin;

          # App bundle paths (used in build and apps)
          appBundleName = "Arto.app";
          appExecutableName = "arto"; # lowercase executable name
          dxBundlePath = "target/dx/${packageMeta.pname}/bundle/macos/bundle/macos";

          renderer-assets = pkgs.stdenvNoCC.mkDerivation (finalAttrs: {
            pname = "${packageMeta.pname}-renderer-assets";
            inherit (packageMeta) version;
            src = ./renderer;

            nativeBuildInputs = [
              pkgs.nodejs-slim
              pkgs.pnpm_9
              pkgs.pnpmConfigHook
            ];

            pnpmDeps = pkgs.pnpm_9.fetchDeps {
              inherit (finalAttrs) pname version src;
              # To update this hash when renderer dependencies change:
              # 1. Change hash to: lib.fakeHash or ""
              # 2. Run: nix build .#renderer-assets
              # 3. Copy the expected hash from error message
              # 4. Update hash value below
              hash = "sha256-te2RlBOaftaTvBrrXLyuS0fcv0u94m1htAjnKuU1LwQ=";
              fetcherVersion = 2;
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
                (root + /assets)
                (root + /Dioxus.toml)
              ];
            };
            strictDeps = true;
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

          arto = craneLib.buildPackage (
            commonArgs
            // {
              inherit (packageMeta) pname version;
              inherit cargoArtifacts;

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
                ];

              postPatch = ''
                mkdir -p assets/dist
                cp -r ${renderer-assets}/* assets/dist/

                # Dioxus.toml references "../extras/mac/arto-app.icns" and "../LICENSE"
                # Copy them from project root to satisfy relative path requirements
                cp -r ${./extras} ../extras
                cp ${./LICENSE} ../LICENSE
              '';

              # Use buildPhaseCargoCommand instead of cargoBuildCommand because crane's
              # additional build argument `--message-format` cannot be passed to dioxus-cli properly.
              # https://crane.dev/API.html#cranelibbuildpackage
              buildPhaseCargoCommand = ''
                dx bundle --release --platform desktop --package-types macos
              '';

              # The build output is a macOS .app bundle, and crane cannot infer the install
              # destination, so we manually install without capturing cargoBuildLog in buildPhase.
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
          appBundleName = "Arto.app";
          appExecutableName = "arto";
        in
        {
          default = {
            type = "app";
            program = "${arto}/Applications/${appBundleName}/Contents/MacOS/${appExecutableName}";
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
              pkgs.pnpm_9

              # Build automation
              pkgs.just
            ];

            # Workaround: Nix sets DEVELOPER_DIR to its apple-sdk, which breaks `just build` dmg creation.
            # https://github.com/NixOS/nixpkgs/issues/355486
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
