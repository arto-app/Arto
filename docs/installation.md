# Installation

macOS is where Arto is developed and tested. Linux builds are published and
work, but see [Platform support](#platform-support) below before relying on
them.

## macOS

Install with the [Homebrew] tap. Arto is not signed or notarized with an Apple
Developer ID, so the quarantine attribute has to be removed after installing —
see [homebrew-tap] for why.

```sh
brew install --cask arto-app/tap/arto
xattr -dr com.apple.quarantine /Applications/Arto.app
```

> [!TIP]
> **Quick Look preview not showing?** macOS normally registers the Quick Look extension the first time you launch Arto. If pressing Space on a Markdown file still shows no preview — or a stale one right after an upgrade — register the extension manually and refresh the cache:
>
> ```sh
> pluginkit -a /Applications/Arto.app/Contents/PlugIns/ArtoQuickLook.appex
> qlmanage -r && qlmanage -r cache
> ```

## Linux

On Debian and Ubuntu, download the `.deb` matching your architecture from the [releases] page and install it with `apt`, which pulls in the GTK/WebKit libraries it declares:

```sh
sudo apt install ./arto_<version>_amd64.deb
```

On every other distribution — Fedora, openSUSE, Arch — download the `.AppImage` instead, make it executable and run it:

```sh
chmod +x arto_<version>_x86_64.AppImage
./arto_<version>_x86_64.AppImage
```

The AppImage needs WebKitGTK 4.1 present on the system; on Fedora that is `sudo dnf install webkit2gtk4.1`.

Both artifacts are built on Ubuntu 24.04, so they require glibc 2.39 or newer (Ubuntu 24.04+, Debian 13+, Fedora 40+). On older distributions, build from source or use Nix.

## A single binary

Every release also carries the application as one executable, for Linux and
Windows, when a package is more ceremony than you want. Download the one for
your machine from the [releases] page and run it from wherever you put it —
the stylesheet, the scripts and the icons are compiled into it, so there is
nothing to install beside it.

```sh
chmod +x arto-linux-x86_64
./arto-linux-x86_64 README.md
```

It is the same application, without what an installer arranges around it: no
menu entry, no file associations, and no `arto` on your `PATH` unless you put
it there. The Linux binary still needs WebKitGTK 4.1 on the system, exactly as
the `.deb` and the AppImage do.

macOS has no such download on purpose. Most of what makes Arto worth
installing there — the Finder associations and the Quick Look preview — is
carried by the app bundle rather than by the executable inside it, so the DMG
is the whole story.

## Nix

[Nix] works on both macOS and Linux. To try Arto without installing it:

```sh
nix run github:arto-app/Arto
```

For a permanent installation, use [nix-darwin] or [home-manager]. Add the flake input:

```nix
arto.url = "github:arto-app/Arto";
```

Then add the package to `environment.systemPackages` (nix-darwin) or `home.packages` (home-manager):

```nix
environment.systemPackages = [ inputs.arto.packages.${system}.default ];
```

The standalone page renderer is a separate package, `arto-page`, for machines that only need `arto page` without the app:

```sh
nix run github:arto-app/Arto#arto-page -- README.md > README.html
```

## Platform support

| Platform | Status |
| --- | --- |
| macOS | Supported. Developed and tested here, and the only platform with Quick Look integration. |
| Linux | Experimental. Builds are published and CI runs the test suite, but the desktop integration gets far less real use. |
| Windows | Experimental. CI builds and tests it, and a release carries an installer and a single binary when that build succeeds, but almost nobody runs it. |

Bug reports for the experimental platforms are welcome, and so are PRs.

## After installing

Launch Arto to see the welcome screen, which lists the keyboard shortcuts and
how to get started.

Homebrew, the `.deb` and Nix also put an `arto` command on your `PATH`. The
AppImage and the single binary do not — each is one self-contained file, so run
it by its own path instead. Either way, see [CLI usage](./cli.md) for what you
can hand it.

[Homebrew]: https://brew.sh/
[homebrew-tap]: https://github.com/arto-app/homebrew-tap
[releases]: https://github.com/arto-app/Arto/releases
[Nix]: https://nixos.org/
[nix-darwin]: https://github.com/nix-darwin/nix-darwin
[home-manager]: https://github.com/nix-community/home-manager
