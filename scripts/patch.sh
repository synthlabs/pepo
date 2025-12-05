#!/bin/bash

set -ex

ROOT=$(git rev-parse --show-toplevel)

vergo -project-root ${ROOT} -debug -update
sumry -project-root ${ROOT} -debug -update
