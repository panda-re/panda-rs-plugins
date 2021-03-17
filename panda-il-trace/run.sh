#/bin/bash

DEF_PROC="dhclient"
DEF_REC="../test"
PROC_NAME=${1:-$DEF_PROC}
REC_NAME=${2:-$DEF_REC}
RR_FILE="$REC_NAME-rr-snp"
RR_ND_LOG="$REC_NAME-rr-nondet.log"

if [ -z "$1" ]; then
    echo "No process name specified, defaulting to '$DEF_PROC'"
fi

if [[ ! -f "$RR_FILE" ]] || [[ ! -f "$RR_ND_LOG" ]]; then
    echo -e "Mising replay files, please run 'take_test_recording.py'"
    exit 1
fi

cargo build --release && \
    cp ../target/release/libpanda_il_trace.so $PANDA_PATH/x86_64-softmmu/panda/plugins/panda_il_trace.so && \
    $PANDA_PATH/x86_64-softmmu/panda-system-x86_64 -os "linux-64-ubuntu:4.15.0-72-generic-noaslr-nokaslr" -replay $REC_NAME -panda il_trace:proc_name=$PROC_NAME,debug=1,trace_lib=1,pretty_json=1 -m 1G
