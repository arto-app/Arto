#!/usr/bin/env bash
# Wrapper for xattr that always succeeds without doing anything.
#
# dioxus-cli calls 'xattr -cr <path>' to remove quarantine attributes
# (com.apple.quarantine) which macOS adds to files downloaded from the internet.
#
# In Nix build:
# - All files are built locally, not downloaded → no quarantine attributes exist
# - xattr operations may fail due to sandbox permission restrictions
# - Therefore, skipping xattr is both safe and necessary
#
# Note: This is different from codesign, which is required for execution.
#       Quarantine removal (xattr) ≠ Code signing (codesign)

# Always succeed without doing anything
exit 0
