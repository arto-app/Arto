set windows-shell := ["pwsh.exe", "-NoProfile", "-Command"]

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

open: desktop::open

[macos]
install: desktop::install
