#!/usr/bin/env bash
# Make the self-signed identity the macOS app is signed with, "Arto Release",
# for the secrets of the GitHub environment "release": MACOS_SIGNING_P12
# (identity.p12 in base64) and MACOS_SIGNING_PASSWORD, written into a new
# folder. Every release must be signed by the same one, or macOS forgets what
# users allowed the app — the keychain items it may read among them.
#
#   release-identity.sh <folder to create>
set -euo pipefail

out="$1"
name="Arto Release"
# The folder holds the private key and the password that opens it.
umask 077
mkdir "$out"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cat >"$work/openssl.cnf" <<CNF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $name
[ext]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
CNF
# The system's LibreSSL writes a PKCS#12 file the keychain can import;
# OpenSSL 3 encrypts it in a way the keychain cannot read.
openssl=/usr/bin/openssl
"$openssl" req -x509 -newkey rsa:2048 -nodes -days 7300 \
  -config "$work/openssl.cnf" -keyout "$work/key.pem" -out "$work/cert.pem" 2>/dev/null
"$openssl" rand -hex 24 >"$out/password"
"$openssl" pkcs12 -export -passout "file:$out/password" \
  -inkey "$work/key.pem" -in "$work/cert.pem" -out "$out/identity.p12"
/usr/bin/base64 -i "$out/identity.p12" >"$out/identity.p12.base64"
"$openssl" x509 -in "$work/cert.pem" -noout -fingerprint -sha256
cat <<EOF
Register these as secrets of the GitHub environment "release", keep them
where import-identity.sh can be given them, then delete $out:
  MACOS_SIGNING_P12       the content of $out/identity.p12.base64
  MACOS_SIGNING_PASSWORD  the content of $out/password
EOF
