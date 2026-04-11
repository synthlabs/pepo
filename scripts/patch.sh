#!/bin/bash

set -e

ROOT=$(git rev-parse --show-toplevel)

vergo -project-root "${ROOT}" -debug -update
VERSION=$(jq -r '.version' "${ROOT}/package.json")
sumry -project-root "${ROOT}" -debug -update

pnpm tauri build --no-sign

git add \
  "${ROOT}/package.json" \
  "${ROOT}/src-tauri/Cargo.toml" \
  "${ROOT}/src-tauri/Cargo.lock" \
  "${ROOT}/src-tauri/tauri.conf.json" \
  "${ROOT}/SUMRY.md" \
  "${ROOT}/archive/"

git commit -m "chore(updater): version bump ${VERSION}"
