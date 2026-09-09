mod arto 'crates/arto'
mod frontend

[private]
default:
  @just --list

setup: frontend::setup

fmt: arto::fmt frontend::fmt

check: frontend::assets arto::check frontend::check

test: frontend::assets arto::test frontend::test

verify: fmt check test

# Links in Markdown files (offline, see lychee.toml) and repository paths
# named in documentation; the same checks the CI docs job runs.
docs:
  lychee --config lychee.toml '**/*.md'
  perl .github/scripts/check-doc-paths.pl

# Workflow files: syntax and expression errors, then security smells.
workflows:
  actionlint
  zizmor --min-severity low --config .github/zizmor.yml .

clean: arto::clean frontend::clean

# Vite rebuilds the frontend bundle in the background while `dx serve` runs
# the app with hot reload. The bundle is built up front only when it is
# missing, so restarting the loop stays fast.
#
# Development loop: Vite watch + dx serve
dev:
  #!/usr/bin/env bash
  set -euo pipefail
  root="{{justfile_directory()}}"
  cd "$root/frontend"
  # Respect VITE_OUT_DIR so the existence check matches where Vite writes.
  dist="${VITE_OUT_DIR:-$root/crates/arto/assets/frontend}"
  if [ ! -f "$dist/main.js" ] || [ ! -f "$dist/main.css" ]; then
    echo "Building frontend bundle..."
    pnpm exec vite build --mode development --minify false
  else
    echo "Frontend bundle found, skipping initial build..."
  fi
  pnpm run dev --logLevel silent >/dev/null 2>&1 &
  vite_pid=$!
  trap 'kill "$vite_pid" 2>/dev/null || true' EXIT
  cd "$root/crates/arto"
  # macOS will not launch an app bundle that carries no seal of its own, so
  # the bundle `dx` assembles has to be signed with the same ad-hoc identity
  # the release build uses. `platform/macos/bundle/dev.entitlements` says why
  # it grants nothing.
  if [ "$(uname -s)" = "Darwin" ]; then
    dx serve --codesign true --apple-team-id=- \
      --apple-entitlements "$root/platform/macos/bundle/dev.entitlements"
  else
    dx serve
  fi

build: frontend::assets arto::build

# What an installer sets up — file associations, a desktop entry, the Quick
# Look extension — is exactly what this does not carry. Everything the app
# itself needs is compiled in, so the binary runs from wherever it is put.
#
# The application as one executable, at target/release/arto
standalone: frontend::assets arto::standalone

# Gate for release artifacts; see the arto recipe for what it rejects.
[macos]
verify-bundle: arto::verify-bundle

[linux]
verify-bundle: arto::verify-bundle

open: arto::open

[macos]
install: arto::install
