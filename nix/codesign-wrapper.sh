#!/usr/bin/env bash
# Wrapper for codesign that handles directories by recursively signing files.
#
# Purpose: macOS requires .app bundles to be code-signed for execution,
#          especially on ARM64 (Apple Silicon). Ad-hoc signing (-s "-") is sufficient.
#
# Problem: sigtool's codesign only accepts individual files as arguments,
#          but dioxus-cli calls codesign on entire .app directories.
#
# Solution: This wrapper detects directory targets and recursively signs
#           all contained files individually.
#
# Usage: This script is used by Nix build to replace the codesign command.
#        The @CODESIGN_BIN@ placeholder is replaced with the actual sigtool path.
#
# Note: This is different from xattr, which removes download quarantine attributes.
#       Code signing (codesign) ≠ Quarantine removal (xattr)

set -euo pipefail

args=()
target=""

# Parse arguments to find the target path (last non-option argument)
for arg in "$@"; do
  if [[ "$arg" != -* ]] && [[ -e "$arg" ]]; then
    target="$arg"
  fi
  args+=("$arg")
done

if [[ -d "$target" ]]; then
  # For directories, recursively sign all files
  while IFS= read -r -d $'\0' f; do
    # Build new args with the file instead of the directory
    file_args=()
    for arg in "${args[@]}"; do
      if [[ "$arg" == "$target" ]]; then
        file_args+=("$f")
      else
        file_args+=("$arg")
      fi
    done
    @CODESIGN_BIN@ "${file_args[@]}" 2>/dev/null || true
  done < <(find "$target" -type f -print0)
else
  exec @CODESIGN_BIN@ "$@"
fi
