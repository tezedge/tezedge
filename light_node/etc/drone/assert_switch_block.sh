#!/usr/bin/env bash

URL=$1
THRESHOLD=$2
LOG_FILE=$3
SWITCH_BLOCK=$4

while true
do
	LEVEL=$(curl -s $URL | jq .level)
    echo "Head at $LEVEL"

    if [[ $LEVEL -gt $SWITCH_BLOCK ]]; then
        break
    fi

	sleep 1
done

echo "Switch block was applied"

# grep "validation_result_message: lvl $SWITCH_BLOCK," $LOG_FILE #| tr ',' '\n'

APPLY_TIME=$(grep "validation_result_message: lvl $SWITCH_BLOCK," $LOG_FILE | tr ',' '\n' | grep "protocol_call_elapsed" | awk '{print $2}')

if [[ $APPLY_TIME -gt $THRESHOLD ]]; then
    echo "Block application too long: $APPLY_TIME ms"
    exit 1
fi