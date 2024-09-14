#!/bin/bash

ROOT=$(git rev-parse --show-toplevel)
VERGO_BIN="${ROOT}/bin/vergo"
VERGO_SRC="${ROOT}/scripts/vergo"

echo "Project root ${ROOT}"
echo "Vergo bin ${VERGO_BIN}"
echo "Vergo src ${VERGO_SRC}"

set -ex

# if [[ ! -f VERGO_BIN ]]; then
#     echo "vergo binary missing, building it..."
    pushd $VERGO_SRC

    go build -o $VERGO_BIN ./main.go

    popd
# fi

EXTRA_ARGS="-project-root ${ROOT} -debug"

${VERGO_BIN} $(echo "${EXTRA_ARGS} $@" | awk '{$1=$1};1')
