[private]
default:
  @just --list

[private]
setup:
  @cd renderer && pnpm install

assets:
  @cd renderer && pnpm run build

fmt: setup
  @cd desktop && cargo fmt --all
  @cd renderer && pnpm run fmt

check: setup assets
  @cd renderer && pnpm run check
  @cd desktop && cargo check --all-targets --all-features
  @cd desktop && cargo clippy --all-targets --all-features -- -D warnings

test: setup assets
  @cd desktop && cargo test --all-features --all-targets

verify: fmt check test

clean:
  @cd renderer && pnpm cache delete
  @cd desktop && cargo clean

dev: setup
  @bash -c ./scripts/dev.sh

build: setup assets
  @# Clean stale hashed assets from previous builds (Dioxus doesn't GC them)
  @rm -rf desktop/target/dx/arto/release/macos/Arto.app/Contents/Resources/assets
  @rm -rf desktop/target/dx/arto/bundle/macos/bundle/macos/Arto.app/Contents/Resources/assets
  @cd desktop && dx bundle --release --macos

open:
  @./desktop/target/dx/arto/bundle/macos/bundle/macos/Arto.app/Contents/MacOS/arto

install:
  @rm -rf /Applications/Arto.app
  @cp -af desktop/target/dx/arto/bundle/macos/bundle/macos/Arto.app /Applications/.
