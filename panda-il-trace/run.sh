#/bin/bash

RR_FILE=test-rr-snp
RR_ND_LOG=test-rr-nondet.log
DEF_PROC="dhclient"

if [ -z "$1" ]; then
    echo "No process name specified, defaulting to '$DEF_PROC'"
fi

cd ..
if [[ ! -f "$RR_FILE" ]] || [[ ! -f "$RR_ND_LOG" ]]; then
    echo -e "Mising test replay files, please run 'take_test_recording.py'"
    exit 1
fi
cd -

cargo build && \
    cp ../target/debug/libpanda_il_trace.so $PANDA_PATH/x86_64-softmmu/panda/plugins/panda_il_trace.so && \
    $PANDA_PATH/x86_64-softmmu/panda-system-x86_64 -os "linux-64-ubuntu:4.15.0-72-generic-noaslr-nokaslr" -replay ../test -panda il_trace:proc_name=${1:-$DEF_PROC} -m 1G
