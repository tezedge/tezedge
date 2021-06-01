#!/usr/bin/env bash

URL=$1
THRESHOLD=$2
LOG_FILE=$3
SWITCH_BLOCK=$4

SCRIPTPATH="$( cd -- "$(dirname "$0")" >/dev/null 2>&1 ; pwd -P )"

COUNT=0
while true
do
	LEVEL=$(curl -s $URL | jq .level)
    echo "Head at $LEVEL"

    if [[ $LEVEL -gt $SWITCH_BLOCK ]]; then
        break
    fi

    # timeout for node to reach the desired block
    if [[ $COUNT == 600 ]]; then
        exit 1
    fi

    ((++COUNT))
	sleep 1
done

echo "Switch block was applied"

# Check if the file exists, the timeout it low, because once the node starts responding to the RPC above, the log file should be already create
$SCRIPTPATH/wait_file $LOG_FILE 1

# We get the application time by parsing the log file produced by the node
APPLY_TIME=$(grep "validation_result_message: lvl $SWITCH_BLOCK," $LOG_FILE | tr ',' '\n' | grep "protocol_call_elapsed" | awk '{print $2}')

if [[ $APPLY_TIME -gt $THRESHOLD ]]; then
    echo "Block application too long: $APPLY_TIME ms"
    exit 1
else
    echo "Block application within threshold: $APPLY_TIME ms"
    exit 0
fi

exit 1