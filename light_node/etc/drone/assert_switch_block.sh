#!/usr/bin/env bash

URL=$1
THRESHOLD=$2
LOG_FILE=$3
SWITCH_BLOCK=$4

SCRIPTPATH="$( cd -- "$(dirname "$0")" >/dev/null 2>&1 ; pwd -P )"

COUNT=0
while true
do  
    # timeout for node to reach the desired block
    if [[ $COUNT == 900 ]]; then
        echo "Block with protocol switch was not applied in the defined timeout "
        exit 1
    fi

    ((++COUNT))

	LEVEL=$(curl -s $URL | jq .level)
    
    if [[ -z "$LEVEL" ]]; then
        echo "Waiting for node"
    else
        echo "Head at $LEVEL"
    fi

    if [[ $LEVEL -gt $SWITCH_BLOCK ]]; then
        break
    fi

	sleep 1
done

echo "Protocol switch block was applied"

# Check if the file exists, the timeout it low, because once the node starts responding to the RPC above, the log file should be already create
$SCRIPTPATH/wait_file.sh $LOG_FILE 1

# We get the application time by parsing the log file produced by the node
APPLY_TIME=$(grep "validation_result_message: lvl $SWITCH_BLOCK," $LOG_FILE | tr ',' '\n' | grep "protocol_call_elapsed" | awk '{print $2}')

if [[ $APPLY_TIME -gt $THRESHOLD ]]; then
    echo "Protocol switch block application too long: $APPLY_TIME ms"
    exit 1
else
    echo "Protocol switch block application within threshold: $APPLY_TIME ms"
    exit 0
fi

exit 1