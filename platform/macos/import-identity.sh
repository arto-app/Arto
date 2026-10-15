#!/usr/bin/env bash
# Import the identity releases are signed with into the login keychain, so
# that `just build` signs local builds with it too, and an installed release
# and a local build keep what macOS let the app do.
#
#   MACOS_SIGNING_P12 (identity.p12 in base64) and MACOS_SIGNING_PASSWORD in
#   the environment, as release-identity.sh made them; import-identity.sh
set -euo pipefail

name="Arto Release"
keychain="$HOME/Library/Keychains/login.keychain-db"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
printf '%s' "$MACOS_SIGNING_P12" | /usr/bin/base64 -D >"$work/identity.p12"
printf '%s' "$MACOS_SIGNING_PASSWORD" >"$work/password"
/usr/bin/openssl pkcs12 -in "$work/identity.p12" -passin "file:$work/password" -nokeys -out "$work/cert.pem" 2>/dev/null
hash="$(/usr/bin/openssl x509 -in "$work/cert.pem" -noout -fingerprint -sha1 | cut -d= -f2 | tr -d :)"

# Matched by hash, not name: a certificate of the same name may be one the
# identity has since replaced, or lack its private key.
if security find-identity -p codesigning "$keychain" | grep -q "$hash"; then
  echo "\"$name\" is already in the login keychain"
  exit 0
fi
if security find-certificate -c "$name" "$keychain" >/dev/null 2>&1; then
  echo "the login keychain holds a \"$name\" other than the one given; delete it in Keychain Access first" >&2
  exit 1
fi
security import "$work/identity.p12" -k "$keychain" -P "$MACOS_SIGNING_PASSWORD" -T /usr/bin/codesign
echo "imported \"$name\" into the login keychain"
