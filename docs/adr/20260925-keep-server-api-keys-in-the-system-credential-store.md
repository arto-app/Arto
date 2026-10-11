# Keep server API keys in the system credential store

Status: Accepted

## Context

A lens may ask a server that wants an API key. `config.json` is often kept in
a dotfiles repository, and an app opened from the Finder or the Dock does not
see the shell's environment.

## Decision

An API key is kept in the system's credential store under its endpoint —
never in `config.json`, and never read from the environment. A lens may name
a command that prints the key instead.

## Alternatives considered

- A key field in `config.json`. It would end up in a repository.
- An environment variable. An app started from the Finder or the Dock does
  not see it, so it would work only when Arto is started from a shell.

## Consequences

- Every lens that asks the same endpoint uses the one key.
- `config.json` can be shared without leaking a key.
