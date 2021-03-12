#!/bin/bash

source /scripts/common.sh
source /scripts/bootstrap-helm.sh


run_tests() {
    echo Running tests...

    wait_pod_ready otv-candidate-verifier-api
}

teardown() {
    helm delete otv-candidate-verifier-api
}

main(){
    if [ -z "$KEEP_W3F_POLKADOT_CHALLENGER" ]; then
        trap teardown EXIT
    fi

    /scripts/build-helmfile.sh

    run_tests
}

main
