#!/bin/sh

set -e

if [ -z "$TEZOS_ENDPOINT" ]; then
    echo "Tezos endpoint \$TEZOS_ENDPOINT is not specified" >&2
    exit 1
fi

if [ -z "$TEZOS_ADDRESSES" ]; then
    echo "Tezos addresses \$TEZOS_ADDRESSES is not specified" >&2
    exit 1
fi

LEVEL_THRESHOLD=${LEVEL_THRESHOLD:-1}
TEZOS_AMOUNT=${TEZOS_AMOUNT:-1}

dir=$(mktemp -d tztx.XXXXXX)

cleanup() {
    set +e
    for id in $(cat $dir/tx.list); do
        kill $(cat $dir/$id.pid) >/dev/null 2>&1
    done
    rm -rf $dir
}

trap cleanup INT TERM HUP

error() {
    echo "!!! ERROR !!!: " $*
    echo "====================="
    echo ">>> stdout for $id"
    cat $dir/$id.out
    echo ">>> stderr for $id"
    echo "====================="
    awk '/^Usage:$/ { exit } { print }' $dir/$id.err
    echo "====================="
}

fail() {
    echo "!!! FAIL  !!!: " $*
    echo "====================="
    echo ">>> stdout for $id"
    cat $dir/$id.out
    echo ">>> stderr for $id"
    echo "====================="
    awk '/^Usage:$/ { exit } { print }' $dir/$id.err
    echo "====================="
}

client() {
    tezos-client -E "$TEZOS_ENDPOINT" $*
}

level () {
    # TODO use jq
    client rpc get /chains/main/blocks/head/header | grep '"level":' | sed -e 's/.*"level": \([[:digit:]]*\),.*/\1/'
}

bootstrapped() {
    while true; do
        if client bootstrapped >& /dev/null; then
            break
        fi
        echo ">>> The node is not bootstrapped"
        sleep 10
    done
    echo ">>> The node is bootstrapped"
}

wait_for_next_level() {
    level=$(level)
    while [ "$level" -eq "$(level)" ] ; do
        sleep 5
    done
}

transfer () {
    amount=$1
    from=$2
    to=$3

    id=$from-$to

    out="$dir/$id.out"
    err="$dir/$id.err"
    rm -f $dir/$id.res
    #echo ">>> Starting transaction of $amount from $from to $to"
    if client transfer "$amount" from "$from" to "$to" >$out 2>$err; then
        #echo ">>> ok $id"
        echo ok > $dir/$id.res
    else
        burn_cap=$(tail -n 1 $err | grep "Use \`--burn-cap" | sed -e "s/.*\`\(--burn-cap [0-9.]*\)\`.*/\1/")
        if [ -n "$burn_cap" ]; then
            #echo ">>> Restarting transaction of $amount from $from to $to with $burn_cap"
            if client transfer "$amount" from "$from" to "$to" $burn_cap >$out 2>$err; then
                #echo ">>> ok $id with $burn_cap"
                echo ok > $dir/$id.res
            else
                #echo ">>> err $id with $burn_cap"
                echo err > $dir/$id.res
            fi
        else
            #echo ">>> err $id"
            echo err > $dir/$id.res
        fi
    fi
}

transfer_many() {
    amount=$1
    from=$2
    shift 2
    num=0
    rm -f $dir/tx.list
    for to in $* $from; do
        transfer $amount $from $to &
        id=$from-$to
        echo $! > $dir/$id.pid
        echo $id >> $dir/tx.list
        num=$((num+1))
        from=$to
    done

    echo "Started $num transaction(s)"
}

verify_transactions() {
    ok=0
    fails=0
    errors=0
    for id in $(cat $dir/tx.list); do
        if [ -f $dir/$id.res ]; then
            case $(cat $dir/$id.res) in
                ok)
                    ok=$((ok + 1))
                ;;
                err)
                    fails=$((fails + 1))
                    fail $id "Transaction is unsuccessfull"
                    ;;
                *)
                    errors=$((errors + 1))
                    error $id "Unexpected result for transaction"
                    ;;
            esac
        elif [ -d "/proc/$(cat $dir/$id.pid)" ]; then
            fails=$((fails + 1))
            fail $id "Transaction is not completed within threshold of $LEVEL_THRESHOLD"
            kill $(cat $dir/$id.pid)
        else
            errors=$((errors + 1))
            error $id "Transaction is completed but not properly reported"
        fi
    done
    echo "Summary (Ok/Fails/Errors): $ok/$fails/$errors"
}

transfer_and_verify() {
    level=$(level)
    target_level=$((level + LEVEL_THRESHOLD))
    transfer_many $*
    echo ">>> Started periodic transfer on level $level, validation threshold is level $target_level"
    shift
    prev_level=$level
    while true; do
        sleep 5
        new_level=$(level)
        if [ $new_level -gt $prev_level ]; then
            echo ">>> Checking on level $new_level"
            prev_level=$new_level
            incomplete=""
            for id in $(cat $dir/tx.list); do
                if [ ! -f $dir/$id.res ]; then
                    incomplete=$id
                    break
                fi
            done
            if [ -z "$incomplete" ]; then
                echo ">>> All transactions completed on level $new_level, checking them"
                break
            fi
        elif [ $new_level -ge $target_level ]; then
            echo ">>> Block level reached $level, checking transactions"
            sleep 5
            break
        fi
    done
    verify_transactions
    echo ">>> Done"

}

transfer_and_verify "$TEZOS_AMOUNT" $TEZOS_ADDRESSES
