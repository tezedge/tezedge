#!/bin/bash

TEZOS=http://127.0.0.1:18888

wait_for_head() {
  node_url="$1/chains/main/blocks/head/header"
  expected_level=$2
  echo "wait_for_head: $node_url - $expected_level"

  head=$(curl -s --location --connect-timeout 1 --request GET $node_url | jq -S . | jq '.level')
  while [[ $head != $expected_level ]]; do
    head=$(curl -s --location --connect-timeout 1 --request GET $node_url | jq -S . | jq '.level')
    echo "waiting $node_url for head.level $expected_level, actual: $head"
    sleep 1;
  done;

  echo $head
}

start_tezos_node_100() {
  # Import snapshot
  rm -rf /home/dev/memprof/tezos-delphi
  /home/dev/tezos/tezos-node config init --network delphinet --data-dir /home/dev/memprof/tezos-delphi --no-bootstrap-peers
  /home/dev/tezos/tezos-node snapshot import /home/dev/memprof/delphi.100.full --data-dir /home/dev/memprof/tezos-delphi  --reconstruct --network delphinet --history-mode full
  # Copy identity
  cp /home/dev/memprof/delphi_data/identity.json /home/dev/memprof/tezos-delphi
  # Run node
  nohup /home/dev/tezos/tezos-node run --data-dir /home/dev/memprof/tezos-delphi --history-mode archive --rpc-addr 0.0.0.0:18888 --net-addr 0.0.0.0:8888 --network delphinet --no-bootstrap-peers &> nohup-tezos.out &
  export TEZOS_PID=$!
  echo -n $TEZOS_PID > /home/dev/memprof/tezos-delphi/pidko
  echo "Tezos node (100) is ready...$TEZOS_PID"
}

start_tezos_node_6000() {
  # Import snapshot
  rm -rf /home/dev/memprof/tezos-delphi-6000
  /home/dev/tezos/tezos-node config init --network delphinet --data-dir /home/dev/memprof/tezos-delphi-6000 --no-bootstrap-peers
  /home/dev/tezos/tezos-node snapshot import /home/dev/memprof/delphi.6000.full --data-dir /home/dev/memprof/tezos-delphi-6000  --reconstruct --network delphinet --history-mode full
  # Copy identity
  cp /home/dev/memprof/delphi_data/identity.json /home/dev/memprof/tezos-delphi-6000
  # Run node
  nohup /home/dev/tezos/tezos-node run --data-dir /home/dev/memprof/tezos-delphi-6000 --history-mode archive --rpc-addr 0.0.0.0:18888 --net-addr 0.0.0.0:8888 --network delphinet --no-bootstrap-peers &> nohup-tezos.out &
  export TEZOS_PID=$!
  echo -n $TEZOS_PID > /home/dev/memprof/tezos-delphi-6000/pidko
  echo "Tezos node (6000) is ready...$TEZOS_PID"
}

start_tezedge_node() {
  rm -rf /home/dev/memprof/tezedge-delphi/tezos_data
  rm -rf /home/dev/memprof/tezedge-delphi/tezedge_data

#  export LD_LIBRARY_PATH=/home/dev/tezedge/tezos/sys/lib_tezos/artifacts

  sudo

#LD_PRELOAD=/home/dev/tezedge/libmemory_profiler.so /home/dev/tezedge/target/release/light-node \
  LD_LIBRARY_PATH=/home/dev/tezedge/tezos/sys/lib_tezos/artifacts /home/dev/tezedge/target/release/light-node \
    --identity-expected-pow 26.0 \
    --tezos-data-dir "/home/dev/memprof/tezedge-delphi/tezos_data" \
    --bootstrap-db-path "/home/dev/memprof/tezedge-delphi/tezedge_data" \
    --disable-bootstrap-lookup \
    --peers 127.0.0.1:8888 \
    --peer-thresh-low 1 --peer-thresh-high 1 \
    --identity-file "/home/dev/memprof/tezedge-delphi/identity.json" \
    --network "delphi" \
    --protocol-runner "/home/dev/tezedge/target/release/protocol-runner" \
    --init-sapling-spend-params-file "/home/dev/tezedge/tezos/sys/lib_tezos/artifacts/sapling-spend.params" \
    --init-sapling-output-params-file "/home/dev/tezedge/tezos/sys/lib_tezos/artifacts/sapling-output.params" \
    --p2p-port 9999 --rpc-port 19999 \
    --websocket-address 0.0.0.0:4927 \
    --tokio-threads 0 \
    --ocaml-log-enabled false \
    --actions-store-backend none \
    --log-level debug \
    --log-format simple \
    &> nohup-tezedge.out &

  export TEZEDGE_PID=$!
  echo -n $TEZEDGE_PID > /home/dev/memprof/tezedge-delphi/pidko
  echo "Tezedge node is ready...$TEZEDGE_PID"
}

dump_memory() {
  tezedge_pidko=$(cat /home/dev/memprof/tezedge-delphi/pidko)

  while [[ true ]]; do
    mem=$(cat /proc/$tezedge_pidko/statm | awk '{print $2}')
    echo $mem >> /home/dev/memprof/tezedge-delphi/$tezedge_pidko.mem

    heaptrack_pidwrite=$(pgrep -P $tezedge_pidko -a | grep heaptrack | grep tezos_writeable | awk '{print $1}')
    pidwrite=$(pgrep -P $heaptrack_pidwrite -a | grep tezos_writeable | awk '{print $1}')
    if [[ "$pidwrite" =~ ^[0-9]+$ ]]; then
      mem=$(cat /proc/$pidwrite/statm | awk '{print $2}')
      echo $mem >> /home/dev/memprof/tezedge-delphi/$tezedge_pidko-$pidwrite-w.mem
    fi

    heaptrack_pidread=$(pgrep -P $tezedge_pidko -a | grep heaptrack | grep tezos_readonly | awk '{print $1}')
    pidread=$(pgrep -P $heaptrack_pidread -a | grep tezos_readonly | awk '{print $1}')
    if [[ "$pidread" =~ ^[0-9]+$ ]]; then
      mem=$(cat /proc/$pidread/statm | awk '{print $2}')
      echo $mem >> /home/dev/memprof/tezedge-delphi/$tezedge_pidko-$pidread-r.mem
    fi

    sleep 0.5
 done;
}

stop() {
  tezos_pidko=$(cat /home/dev/memprof/tezos-delphi/pidko)
  echo "Stopping tezos pids...$tezos_pidko"
  kill -s SIGINT $tezos_pidko;

  tezos6000_pidko=$(cat /home/dev/memprof/tezos-delphi-6000/pidko)
  echo "Stopping tezos-6000 pids...$tezos6000_pidko"
  kill -s SIGINT $tezos6000_pidko;

  tezedge_pidko=$(cat /home/dev/memprof/tezedge-delphi/pidko)
  echo "Stopping tezedge pids...$tezedge_pidko"
  kill -s SIGINT $tezedge_pidko;

  echo "Stopping done"
}

run_100() {
  stop

  start_tezos_node_100
  wait_for_head "127.0.0.1:18888" "100"

  start_tezedge_node
  dump_memory &
  export DUMP_PID=$!
  wait_for_head "127.0.0.1:19999" "100"

  echo "test done"
  stop
  kill $DUMP_PID
}


run_6000() {
  stop

  start_tezos_node_6000
  wait_for_head "127.0.0.1:18888" "6000"

  start_tezedge_node
  dump_memory &
  export DUMP_PID=$!
  wait_for_head "127.0.0.1:19999" "6000"

  echo "test done"
  stop
  kill $DUMP_PID
}

case $1 in

  run_100)
    run_100
    ;;
  run_6000)
    run_6000
    ;;
  start_tezos)
	  start_tezos_node
    ;;
  start_tezedge)
	  start_tezedge_node
    ;;
  stop)
	  stop
    ;;

  *)
    echo "Invalid command '$1'"
    ;;

esac

# ./run.sh sandbox
# ./test.sh start_sandbox_node
# ps -ef | grep runner
# ./test.sh send_transactions /home/dev/tezos/tezos-client
# ps -ef | grep runner
# curl http://trace.dev.tezedge.com:18732/chains/main/mempool/pending_operations
# ./test.sh stop_sandbox_node
