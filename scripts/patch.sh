#!/bin/bash

set -e

ROOT=$(git rev-parse --show-toplevel)

vergo -project-root "${ROOT}" -debug -update
VERSION=$(jq -r '.version' "${ROOT}/package.json")
sumry -project-root "${ROOT}" -debug -update

pushd "${ROOT}/src-tauri"

cargo run --example gen_bindings --features="gen_bindings"

popd

git add \
  "${ROOT}/package.json" \
  "${ROOT}/src-tauri/Cargo.toml" \
  "${ROOT}/Cargo.lock" \
  "${ROOT}/src-tauri/tauri.conf.json" \
  "${ROOT}/SUMRY.md" \
  "${ROOT}/archive/"

git commit -m "chore(updater): version bump ${VERSION}"
