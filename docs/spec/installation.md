# Installation

macOS is where Arto is developed and tested. Linux builds are published and
work, but see [Platform support](#platform-support) below before relying on
them.

## macOS

Releases are built for Apple silicon Macs. Install with the [Homebrew] tap.
Arto is not signed or notarized with an Apple
Developer ID, so the cask removes the quarantine attribute that would otherwise
stop macOS from opening it — see [homebrew-tap] for why.

Every release is signed by the same self-signed certificate, so an update
keeps what you allowed the previous release to do, such as reading the API
keys Arto keeps in the keychain.

```sh
brew install --cask arto-app/tap/arto
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

The AppImage needs WebKitGTK 4.1 installed on the system; the image does not
carry it. Install it first if it is missing:

```sh
sudo dnf install webkit2gtk4.1          # Fedora
sudo zypper install libwebkit2gtk-4_1-0 # openSUSE
sudo pacman -S webkit2gtk-4.1           # Arch
```

Both artifacts are built on Ubuntu 24.04, so they require glibc 2.39 or newer (Ubuntu 24.04+, Debian 13+, Fedora 40+). On older distributions, build from source or use Nix.

## Windows

Download the `Arto_<version>_x64-setup.exe` — or `_arm64-` — from the
[releases] page and run it. It installs Arto, adds it to the Start menu and
registers the Markdown files it opens.

Arto renders through Microsoft's WebView2 runtime. Windows 11 ships it and
Windows 10 usually has it from Edge, but where it is missing the installer
fetches it, so that first install needs a working connection. Nothing after it
does: the app renders offline.

## A single binary

A release also carries the application as one executable, for Linux and
Windows when their build succeeds (see [Platform support](#platform-support)),
when a package is more ceremony than you want. Download the one for
your machine from the [releases] page and run it from wherever you put it —
the stylesheet, the scripts and the icons are compiled into it, so there is
nothing to install beside it.

```sh
chmod +x arto-linux-x86_64
./arto-linux-x86_64 README.md
```

It is the same application, without what an installer arranges around it: no
menu entry, no file associations, and no `arto` on your `PATH` unless you put
it there. The Linux binary still needs WebKitGTK 4.1 on the system, as the
`.deb` and the AppImage do, and the other system libraries the `.deb`
declares, such as `libxdo`; the `.deb` or Nix pulls them in for you. It is
built where the `.deb` is, so it needs the same glibc (see [Linux](#linux)).

On Windows the single binary is the one a shell waits for, so it is the copy
to run from a terminal — see [CLI usage](./cli.md). Double-clicked, it hides
the console it was given.

Both carry the Microsoft C runtime inside them, so neither asks for the Visual
C++ redistributable to be installed first.

macOS has no such download; Arto is installed there as the app bundle, which
carries the Finder associations and the Quick Look preview.

## Nix

[Nix] works on Apple silicon Macs and on Linux. The app bundle it builds on
macOS has no Quick Look preview; install with Homebrew for that. To try Arto
without installing it:

```sh
nix run github:arto-app/Arto
```

For a permanent installation, use [nix-darwin] or [home-manager]. Add the flake input:

```nix
arto.url = "github:arto-app/Arto";          # follows main
arto.url = "github:arto-app/Arto/vX.Y.Z";   # or pins a release, and reports its version
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
| macOS | Supported on Apple silicon. Developed and tested here, and the only platform with Quick Look integration. |
| Linux | Experimental. Builds are published and CI runs the test suite, but the desktop integration gets far less real use. |
| Windows | Experimental. CI builds and tests it, and a release carries an installer and a single binary when that build succeeds, but almost nobody runs it. |

Bug reports for the experimental platforms are welcome, and so are PRs.

## After installing

Launch Arto to see the welcome screen, which lists the keyboard shortcuts and
how to get started.

What each way of installing gives you:

| Installed with | `arto` on `PATH` | Menu entry and file associations | Needs on the system |
| --- | --- | --- | --- |
| Homebrew (macOS) | yes | yes, with Quick Look | — |
| `.deb` | yes | yes | WebKitGTK 4.1, pulled in by `apt` |
| AppImage | no — run it by its path | no | WebKitGTK 4.1 |
| Windows installer | no | yes | WebView2, fetched by the installer |
| Single binary | no — run it by its path | no | Linux: WebKitGTK 4.1 and the libraries the `.deb` declares; Windows: WebView2 |
| Nix | yes | macOS: the app bundle, without Quick Look; Linux: no | — |

See [CLI usage](./cli.md) for what you can hand the `arto` command.

[Homebrew]: https://brew.sh/
[homebrew-tap]: https://github.com/arto-app/homebrew-tap
[releases]: https://github.com/arto-app/Arto/releases
[Nix]: https://nixos.org/
[nix-darwin]: https://github.com/nix-darwin/nix-darwin
[home-manager]: https://github.com/nix-community/home-manager
