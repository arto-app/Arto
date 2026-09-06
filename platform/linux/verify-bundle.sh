#!/usr/bin/env bash
# Reject a Linux artifact that would not run outside the build environment.
#
# An artifact built inside the Nix devShell records an ELF interpreter and a
# RUNPATH under /nix/store. Such a binary cannot even be exec'd on a machine
# without that store path, so the whole artifact is dead on arrival.
#
# The packaged binary is inspected rather than the one in the build tree,
# because the package is what gets published.
#
#   1. The ELF interpreter, NEEDED entries and RUNPATH/RPATH must be free of
#      /nix/store, and no /nix/store string may survive anywhere in the binary.
#      An AppImage carries copies of the host's libraries, so those are checked
#      for the same references too.
#   2. A .deb must declare its shared-library dependencies, so installing it
#      pulls them in instead of leaving the user to hunt them down. An AppImage
#      has no dependency metadata to declare — it carries the libraries
#      linuxdeploy copied in — so this check applies to the .deb only.
#   3. The artifact actually starts, which is the only way to confirm that the
#      interpreter and every NEEDED library really resolve. `--version` exits
#      immediately and needs no display.
set -euo pipefail

usage="usage: platform/linux/verify-bundle.sh <path to .deb or .AppImage> <executable name>"
artifact="${1:?$usage}"
executable="${2:?$usage}"

case "$artifact" in
  *.deb) kind=deb ;;
  *.AppImage) kind=appimage ;;
  *)
    echo "Error: unsupported artifact '$artifact' ($usage)" >&2
    exit 1
    ;;
esac

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT

if [[ "$kind" == "deb" ]]; then
  dpkg-deb --fsys-tarfile "$artifact" | tar -x -C "$workdir"
  root="$workdir"
else
  # `--appimage-extract` unpacks into ./squashfs-root of the *current*
  # directory, so it has to run inside the scratch dir with an absolute path to
  # the artifact. Extraction is used rather than a FUSE mount because CI
  # runners and containers frequently have no /dev/fuse.
  artifact="$(cd "$(dirname "$artifact")" && pwd)/$(basename "$artifact")"
  chmod +x "$artifact"
  (cd "$workdir" && "$artifact" --appimage-extract >/dev/null)
  root="$workdir/squashfs-root"
fi

bin="$root/usr/bin/$executable"
if [[ ! -x "$bin" ]]; then
  echo "Error: $artifact does not contain usr/bin/$executable" >&2
  exit 1
fi

status=0

# Captured before grepping: piping into grep under `pipefail` would let a
# readelf failure masquerade as a clean result.
headers="$(readelf -p .interp "$bin"; readelf -d "$bin")"
embedded="$(strings -a "$bin")"

if refs="$(grep -F /nix/store <<<"$headers")"; then
  echo "Error: usr/bin/$executable depends on the build environment:" >&2
  echo "$refs" >&2
  status=1
elif refs="$(grep -F /nix/store <<<"$embedded" | sort -u)"; then
  echo "Error: usr/bin/$executable still embeds build environment paths:" >&2
  echo "$refs" >&2
  status=1
fi

if [[ "$kind" == "appimage" && -d "$root/usr/lib" ]]; then
  # The header check above is not sufficient for an AppImage: linuxdeploy
  # rewrites RUNPATH to $ORIGIN/../lib and copies every library it resolved on
  # the build host into the AppDir. A devShell build would therefore have its
  # /nix/store RUNPATH laundered out of the headers while the bundled libraries
  # are the store's own. Inspect what was actually shipped alongside it.
  if refs="$(grep -rlF /nix/store "$root/usr/lib")"; then
    echo "Error: bundled libraries come from the build environment:" >&2
    echo "$refs" >&2
    status=1
  fi
fi

if [[ "$kind" == "deb" ]]; then
  depends="$(dpkg-deb -f "$artifact" Depends)"
  if [[ -z "$depends" ]]; then
    echo "Error: $artifact declares no Depends, so installing it does not pull in" >&2
    echo "       the shared libraries the binary needs." >&2
    status=1
  elif command -v apt-cache >/dev/null; then
    # Each declared name must exist in the distro index. A typo would otherwise
    # surface only as an unmet dependency on a user's machine. Alternatives
    # ("a | b") are satisfied by any one of their members, which is how the
    # Ubuntu/Debian package renamings are expressed.
    while IFS= read -r entry; do
      resolved=""
      while IFS= read -r alternative; do
        if apt-cache show "$alternative" >/dev/null 2>&1; then
          resolved=1
          break
        fi
      done < <(tr '|' '\n' <<<"$entry" | perl -lpe 's/\(.*\)//; s/^\s+|\s+$//g')

      if [[ -z "$resolved" ]]; then
        echo "Error: dependency '$entry' matches no package in the distro index" >&2
        status=1
      fi
    done < <(tr ',' '\n' <<<"$depends" | perl -lpe 's/^\s+|\s+$//g' | grep -v '^$')
  fi
fi

if [[ "$status" -ne 0 ]]; then
  exit "$status"
fi

if [[ "$kind" == "deb" ]]; then
  "$bin" --version
  echo "Package verified: no /nix/store references, dependencies declared, executable launches"
else
  # The artifact itself is launched, not the extracted binary: that exercises
  # the AppImage runtime the user actually runs. Extract-and-run for the same
  # no-FUSE reason as above.
  APPIMAGE_EXTRACT_AND_RUN=1 "$artifact" --version
  echo "AppImage verified: no /nix/store references, artifact launches"
fi
