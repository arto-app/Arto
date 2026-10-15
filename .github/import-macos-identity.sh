#!/usr/bin/env bash
# Import the release signing identity into a keychain of its own and put it on
# the search list, so that codesign finds it by name.
#
#   MACOS_SIGNING_P12 (identity.p12 in base64) and MACOS_SIGNING_PASSWORD in
#   the environment; import-macos-identity.sh <keychain to create>
set -euo pipefail

keychain="$1"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
password="$(/usr/bin/openssl rand -hex 24)"

printf '%s' "$MACOS_SIGNING_P12" | /usr/bin/base64 -D >"$work/identity.p12"
security create-keychain -p "$password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$password" "$keychain"
security import "$work/identity.p12" -k "$keychain" -P "$MACOS_SIGNING_PASSWORD" -T /usr/bin/codesign
# Lets codesign use the key without asking.
security set-key-partition-list -S apple-tool:,apple: -s -k "$password" "$keychain" >/dev/null
# shellcheck disable=SC2046 # one keychain path per line, none with spaces
security list-keychains -d user -s "$keychain" $(security list-keychains -d user | tr -d '"')
