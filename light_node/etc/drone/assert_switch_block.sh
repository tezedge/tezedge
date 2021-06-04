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

EXCEEDED=$(grep "Block was applied" $LOG_FILE | tr -d ',' | awk -v t=$THRESHOLD '$9 > t {printf "Application time over threshold: level: %s - %s ms\n", $12, $9}')

EXCEEDED_GREATLY=$(grep "Block was validated with protocol with long processing" $LOG_FILE | tr -d ',' | awk -v t=$THRESHOLD '$23 > t {printf "Extra long application on level %s - %s ms", $15, $23}')

if [[ -z $EXCEEDED && -z $EXCEEDED_GREATLY ]]; then
    echo "All applied blocks within threshold"
    exit 0
else
    echo "FAIL"

    echo "$EXCEEDED"
    echo "$EXCEEDED_GREATLY"

    exit 1
fi
