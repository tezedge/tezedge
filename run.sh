#!/usr/bin/env bash

help() {
  echo -e "Usage: \e[97m$0\e[0m \e[32mMODE\e[0m"
  echo -e "Possible values for \e[32mMODE\e[0m are:"
  echo -e " \e[32mnode\e[0m  Start tezedge node. You can specify additional arguments here."
  echo "       Example: $0 node -v"
}

build_all() {
  export SODIUM_USE_PKG_CONFIG=1
  cargo build
}

run_node() {

  # Default light-node commandline arguments:
  #
  # -B | --bootstrap-db-path
  BOOTSTRAP_DIR=/tmp/tezedge
  # -d | --tezos-data-dir
  TEZOS_DIR=/tmp/tezedge
  # -i | --identity
  IDENTITY_FILE=./light_node/config/identity.json
  # -n | --network
  NETWORK=babylonnet
  # rest of the commandline arguments will end up here
  args=()

  shift # shift past <MODE>

  # The following loop is used to replace default light-node commandline parameters.
  # It also allows to specify additional commandline parameters.
  # Supports '--arg=val' and '--arg val' syntax of the commandline arguments.
  while [ "$#" -gt 0 ]; do
    case $1 in
      -B=*|--bootstrap-db-path=*)
        BOOTSTRAP_DIR="${1#*=}"
        shift
        ;;
      -B|--bootstrap-db-path)
        shift
        BOOTSTRAP_DIR="$1"
        shift
        ;;
      -d=*|--tezos-data-dir=*)
        TEZOS_DIR="${1#*=}"
        shift
        ;;
      -d|--tezos-data-dir)
        shift
        TEZOS_DIR="$1"
        shift
        ;;
      -n=*|--network=*)
        NETWORK="${1#*=}"
        shift
        ;;
      -n|--network)
        shift
        NETWORK="$1"
        shift
        ;;
      *)
        args+=("$1")
        shift
        ;;
    esac
  done


  # cleanup data directory
  rm -rf "$BOOTSTRAP_DIR" && mkdir "$BOOTSTRAP_DIR"
  rm -rf "$TEZOS_DIR" && mkdir "$TEZOS_DIR"

  # protocol_runner needs 'libtezos.o' to run
  export LD_LIBRARY_PATH="${BASH_SOURCE%/*}/tezos/interop/lib_tezos/artifacts"
  # start node
  cargo run --bin light-node -- -d "$TEZOS_DIR" -B "$BOOTSTRAP_DIR" --network "$NETWORK" --protocol-runner ./target/debug/protocol-runner "${args[@]}"
}

case $1 in

  node)
    printf "\033[1;37mRunning Tezedge node\e[0m\n"
    build_all
    run_node "$@"
    ;;

    -h|--help)
      help
      ;;

    *)
      echo "run '$0 --help' to get more info"
      ;;

esac

