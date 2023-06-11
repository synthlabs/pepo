#!/bin/bash

if [[ ! -f .env.secrets ]]; then
    echo "script expects a local secrets file"
    exit 1
fi

for var in $(cat .env.secrets); do
    export $var
done

secrets-init --provider google -- "$@"