#!/usr/bin/env bash
# Reject a Debian package that would not run outside the build environment.
#
# A package built inside the Nix devShell records an ELF interpreter and a
# RUNPATH under /nix/store. Such a binary cannot even be exec'd on a machine
# without that store path, so the whole package is dead on arrival.
#
# The packaged binary is inspected rather than the one in the build tree,
# because the package is what gets published.
#
#   1. The ELF interpreter, NEEDED entries and RUNPATH/RPATH must be free of
#      /nix/store, and no /nix/store string may survive anywhere in the binary.
#   2. The package must declare its shared-library dependencies, so installing
#      it pulls them in instead of leaving the user to hunt them down.
#   3. The binary actually starts, which is the only way to confirm that the
#      interpreter and every NEEDED library really resolve. `--version` exits
#      immediately and needs no display.
set -euo pipefail

deb="${1:?usage: verify-linux-bundle.sh <path to .deb> <executable name>}"
executable="${2:?usage: verify-linux-bundle.sh <path to .deb> <executable name>}"

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT
dpkg-deb --fsys-tarfile "$deb" | tar -x -C "$workdir"

bin="$workdir/usr/bin/$executable"
if [[ ! -x "$bin" ]]; then
  echo "Error: $deb does not contain usr/bin/$executable" >&2
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

depends="$(dpkg-deb -f "$deb" Depends)"
if [[ -z "$depends" ]]; then
  echo "Error: $deb declares no Depends, so installing it does not pull in" >&2
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

if [[ "$status" -ne 0 ]]; then
  exit "$status"
fi

"$bin" --version
echo "Package verified: no /nix/store references, dependencies declared, executable launches"
