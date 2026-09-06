mod arto 'crates/arto'
mod renderer

[private]
default:
  @just --list

setup: renderer::setup

fmt: arto::fmt renderer::fmt

check: renderer::assets arto::check renderer::check

test: renderer::assets arto::test renderer::test

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

clean: arto::clean renderer::clean

# Vite rebuilds the renderer bundle in the background while `dx serve` runs
# the app with hot reload. The bundle is built up front only when it is
# missing, so restarting the loop stays fast.
#
# Development loop: Vite watch + dx serve
dev:
  #!/usr/bin/env bash
  set -euo pipefail
  root="{{justfile_directory()}}"
  cd "$root/renderer"
  # Respect VITE_OUT_DIR so the existence check matches where Vite writes.
  dist="${VITE_OUT_DIR:-$root/crates/arto/assets/dist}"
  if [ ! -f "$dist/main.js" ] || [ ! -f "$dist/main.css" ]; then
    echo "Building renderer artifacts..."
    pnpm exec vite build --mode development --minify false
  else
    echo "Renderer artifacts found, skipping initial build..."
  fi
  pnpm run dev --logLevel silent >/dev/null 2>&1 &
  vite_pid=$!
  trap 'kill "$vite_pid" 2>/dev/null || true' EXIT
  cd "$root/crates/arto"
  dx serve

build: renderer::assets arto::build

# Gate for release artifacts; see the arto recipe for what it rejects.
[macos]
verify-bundle: arto::verify-bundle

[linux]
verify-bundle: arto::verify-bundle

open: arto::open

[macos]
install: arto::install
