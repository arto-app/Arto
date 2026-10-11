# Ship no single executable for macOS

Status: Accepted

## Context

Once everything the app needs was compiled into the binary, a release could
offer one executable to run wherever it is put, and Linux and Windows
releases do.

## Decision

macOS releases carry the app bundle only, not a bare executable.

## Alternatives considered

- Publish the executable on macOS too. Most of what makes Arto worth
  installing there — the Finder associations and the Quick Look preview — is
  carried by the bundle, not by the executable inside it, so a bare binary
  would be a worse install rather than a simpler one.

## Consequences

- On macOS, Arto is installed as `Arto.app`, through the DMG, Homebrew or
  Nix.
