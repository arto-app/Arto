# Leave WebKitGTK out of the AppImage

Status: Accepted

## Context

WebKitGTK renders in helper processes that it finds at an absolute path
compiled into the library. The override that would redirect the lookup is
compiled out of distribution builds, so a bundled copy of the library still
spawned the build host's helpers, and Arto died at its first web process on
every machine that kept WebKitGTK elsewhere or did not have it.

## Decision

The AppImage carries neither WebKitGTK nor the libraries it depends on; they
come from the machine that runs it.

## Alternatives considered

- Bundle the helpers beside the library. Nothing an AppImage can set makes
  the library look for them there.

## Consequences

- The AppImage, like the `.deb` and the single binary, needs WebKitGTK 4.1
  installed on the system.
