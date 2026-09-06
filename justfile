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

dev:
  @bash -c ./scripts/dev.sh

build: renderer::assets arto::build

# Gate for release artifacts; see the arto recipe for what it rejects.
[macos]
verify-bundle: arto::verify-bundle

[linux]
verify-bundle: arto::verify-bundle

open: arto::open

[macos]
install: arto::install
