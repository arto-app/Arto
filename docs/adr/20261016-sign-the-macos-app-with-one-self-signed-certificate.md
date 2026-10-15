# Sign the macOS app with one self-signed certificate

Status: Accepted

## Context

The macOS app was signed ad hoc. An ad-hoc signature names nothing but the
build's own hash, so to macOS every release was a different app: it forgot
what the user had let the previous one do, and asked again — for the API keys
Arto keeps in the keychain among them. Arto has no Apple Developer ID.

## Decision

Every release of the macOS app is signed by the same self-signed certificate,
"Arto Release". Local builds are signed by it too when the keychain has it.

## Alternatives considered

- An Apple Developer ID. It would also let Gatekeeper open the app without the
  user's approval, but it costs a paid membership that the project does not
  hold.
- Stay ad hoc. Every update would keep asking the user again.

## Consequences

- An update keeps what the user let the previous release do.
- Gatekeeper still does not trust the app, so installing it still needs the
  quarantine attribute removed or the user's approval.
- Losing or replacing the certificate makes every user grant the app its
  permissions once more.
