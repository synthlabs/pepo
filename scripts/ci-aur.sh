#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != --check-access ]]; then
  [[ "${PUBLISH_RELEASE:-false}" == true && "${GITHUB_EVENT_NAME:-}" == push && "${GITHUB_REF:-}" == refs/heads/main ]] \
    || { echo 'AUR publication requires a main push with publication enabled' >&2; exit 1; }
fi

bash utils/packaging/tauri/aur/container-publish.sh "$@" \
  --project-root "$PWD" --container-runtime docker \
  --package "$PWD/package/${PACKAGE_FILE:?}" \
  --release-tag "${RELEASE_TAG:?}" \
  --pkgver "${PACKAGE_VERSION:?}" --pkgrel "${PACKAGE_RELEASE:?}" \
  --aur-repository ssh://aur@aur.archlinux.org/pepo-bin.git \
  --ssh-dir "${RUNNER_TEMP:?}/aur-ssh" \
  --git-user-name Jerod --git-user-email jerod@synth.sh
