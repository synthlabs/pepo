#!/bin/bash

ROOT=$(git rev-parse --show-toplevel)

set -ex

EXTRA_ARGS="-project-root ${ROOT} -debug"

sumry $(echo "${EXTRA_ARGS} $@" | awk '{$1=$1};1')
