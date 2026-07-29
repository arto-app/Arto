mod desktop
mod renderer

[private]
default:
  @just --list

setup: renderer::setup

fmt: desktop::fmt renderer::fmt

check: renderer::assets desktop::check renderer::check

test: renderer::assets desktop::test renderer::test

verify: fmt check test

clean: desktop::clean renderer::clean

dev:
  @bash -c ./scripts/dev.sh

build: renderer::assets desktop::build

# Gate for release artifacts; see the desktop recipe for what it rejects.
[macos]
verify-bundle: desktop::verify-bundle

[linux]
verify-bundle: desktop::verify-bundle

open: desktop::open

[macos]
install: desktop::install
