#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

fail() {
  echo "test failure: $*" >&2
  exit 1
}

assert_contains() {
  local value="$1"
  local expected="$2"
  [[ "${value}" == *"${expected}"* ]] || fail "output does not contain ${expected}"
}

assert_not_contains() {
  local value="$1"
  local unexpected="$2"
  [[ "${value}" != *"${unexpected}"* ]] || fail "output unexpectedly contains ${unexpected}"
}

cd -- "${project_root}"

build_output="$(make -n build)"
assert_contains "${build_output}" '/utils/packaging/tauri/build.sh'
assert_contains "${build_output}" "--project-root \"${project_root}\""

internal_output="$(make -n build-internal)"
assert_contains "${internal_output}" '--passthrough-var "ENABLE_INTERNAL"'
assert_contains "${internal_output}" '--passthrough-var "PEPO_LOG"'

install_output="$(
  make -n install \
    TAURI_PACKAGING_INSTALL_ARTIFACT="${project_root}/Sample.deb"
)"
assert_contains "${install_output}" '/utils/packaging/tauri/install.sh'
assert_contains "${install_output}" "--artifact \"${project_root}/Sample.deb\""
assert_not_contains "${install_output}" "${project_root}/packaging/install.sh"

[[ -f packaging/arch/PKGBUILD ]] || fail 'Pepo Arch descriptor is missing'
[[ -f packaging/arch/pepo.desktop ]] || fail 'Pepo desktop descriptor is missing'
[[ -f packaging/aur/PKGBUILD.in ]] || fail 'Pepo AUR descriptor is missing'
[[ ! -e packaging/build.sh ]] || fail 'shared build helper remains in the Pepo packaging directory'
[[ ! -e packaging/install.sh ]] || fail 'shared install helper remains in the Pepo packaging directory'

"${project_root}/utils/packaging/tauri/test.sh"
