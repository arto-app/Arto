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
