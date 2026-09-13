#!/usr/bin/env bash
# Print, one per line, the libraries the AppImage must not carry.
#
# WebKitGTK does its rendering and networking in helper executables —
# WebKitWebProcess, WebKitNetworkProcess — which libwebkit2gtk spawns from an
# absolute path compiled into the library itself (PKGLIBEXECDIR; on Debian and
# Ubuntu /usr/lib/<triple>/webkit2gtk-4.1). Nothing an AppImage can set
# redirects that lookup: the WEBKIT_EXEC_PATH override lives behind
# ENABLE(DEVELOPER_MODE) and is compiled out of every distribution build. Nor
# can an AppImage put the helpers where the lookup goes, since it never writes
# to /usr/lib.
#
# A bundled libwebkit2gtk therefore shadows the system one through the
# executable's RUNPATH while still spawning helpers from the build host's
# path. The application starts, then dies the moment it needs a web process on
# every machine whose WebKitGTK sits somewhere else or is not installed:
#
#   Failed to spawn child process
#   "/usr/lib/aarch64-linux-gnu/webkit2gtk-4.1/WebKitNetworkProcess"
#
# The library and its helpers only work as the matched set a distribution
# installed, so WebKitGTK is left to the system. Its dependency closure goes
# with it rather than just the library: the system WebKitGTK is loaded into
# this process, and whichever copy of GTK or GLib was loaded first is the one
# it resolves its symbols against — a newer WebKitGTK against the older stack
# the build host had would be missing symbols. What the image still carries is
# what belongs to Arto alone.
#
# The closure is read off the build host rather than listed here, so it cannot
# drift from what WebKitGTK actually links.
set -euo pipefail

soname="libwebkit2gtk-4.1.so.0"

library="$(ldconfig -p | perl -lne 'print $1 if m{=> (\S+/'"$soname"')$}' | head -1)"
if [[ -z "$library" ]]; then
  echo "Error: $soname is not installed on this machine, so the libraries to" >&2
  echo "       leave to the system cannot be determined." >&2
  exit 1
fi

# `ldd` reports the whole closure flat, which is exactly the set wanted here.
# Lines without '=>' are the vDSO and the loader, which linuxdeploy excludes on
# its own.
closure="$(ldd "$library" | perl -lane 'print $F[0] if /=>/')"

printf '%s\n' "$soname" "$closure" | sort -u
