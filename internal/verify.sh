#!/usr/bin/env sh
set -eu

ENABLE_INTERNAL=1 pnpm check
ENABLE_INTERNAL=1 cargo check --manifest-path src-tauri/Cargo.toml
git status --short
