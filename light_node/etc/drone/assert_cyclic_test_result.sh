#!/usr/bin/env bash

MAX_LATENCY_THRESHOLD=$1
LOG_FILE=$2

SCRIPTPATH="$( cd -- "$(dirname "$0")" >/dev/null 2>&1 ; pwd -P )"

$SCRIPTPATH/wait_file.sh $LOG_FILE 1
cat $LOG_FILE

# check the last number (Max latency) and compare it agianst the chosen threshold
EXCEEDED=$(tail -n +2 $LOG_FILE | awk -v t=$MAX_LATENCY_THRESHOLD '$NF > t {print $NF}')

echo "$EXCEEDED"
if [[ -z $EXCEEDED ]]; then
    echo "PASS - Latancies within threshold"
    exit 0
else
    echo "FAIL - Latency on some cores are too high! Probably not running on a Real Time Os"
    exit 1
fi
