#!/usr/bin/env bash

help() {
  echo "Usage:"
  echo "run.sh [OPTION]"
  echo "Possible run modes:"
  echo " -n,  --node  Start tezedge node"
}

build_all() {
  export SODIUM_USE_PKG_CONFIG=1
  cargo build
}

run_node() {
  DATA_DIR=/tmp/tezedge
  # cleanup data directory
  rm -rf "$DATA_DIR"
  mkdir "$DATA_DIR"
  # protocol_runner needs 'libtezos.o' to run
  export LD_LIBRARY_PATH="${BASH_SOURCE%/*}/tezos/interop/lib_tezos/artifacts"
  # start node
  cargo run --bin light-node -- -d "$DATA_DIR" -B "$DATA_DIR" -i ./light_node/config/identity.json -t 2 -T 16 -f simple --network babylonnet --protocol-runner ./target/debug/protocol-runner
}

case $1 in

  -n|--node)
    printf "\033[1;37mRun node\e[0m\n";
    build_all
    run_node
    ;;

    -h|--help)
      help
      ;;

    *)
      echo "Missing option"
      echo "run '$0 --help' to get more info"
      ;;

esac

