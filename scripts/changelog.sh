#!/bin/bash

ROOT=$(git rev-parse --show-toplevel)
SUMRY_BIN="${ROOT}/bin/sumry"
SUMRY_SRC="${ROOT}/scripts/sumry"

echo "Project root ${ROOT}"
echo "Sumry bin ${SUMRY_BIN}"
echo "Sumry src ${SUMRY_SRC}"

set -ex

pushd $SUMRY_SRC
go build -o $SUMRY_BIN ./main.go
popd

EXTRA_ARGS="-project-root ${ROOT} -debug"

${SUMRY_BIN} $(echo "${EXTRA_ARGS} $@" | awk '{$1=$1};1')
