#!/usr/bin/env bash
# Copyright (c) SimpleStaking and Tezedge Contributors
# SPDX-License-Identifier: MIT

# stop on first error
set -e

warn_if_not_using_recommended_rust() {
  RUSTC_TOOLCHAIN_VERSION="2021-08-04"

  EXPECTED_RUSTC_VERSION=$(date -d "$RUSTC_TOOLCHAIN_VERSION -1 day" +"%Y-%m-%d")
  RUSTC_VERSION=$(rustc --version)

  if [[ "${RUSTC_VERSION}" != *"${EXPECTED_RUSTC_VERSION}"* ]]; then
    echo -e "\e[33mWarning:\e[0m please use supported rust toolchain version \e[97m$RUSTC_TOOLCHAIN_VERSION\e[0m."
    echo    "Your current version $RUSTC_VERSION."
    echo    "See project README.md on how to install a supported rust toolchain."
  fi
}

help() {
  echo -e "Usage: \e[97m$0\e[0m \e[32mMODE\e[0m"
  echo -e "Possible values for \e[32mMODE\e[0m are:"
  echo -e " \e[32mnode\e[0m       Start tezedge node. You can specify additional arguments here."
  echo    "              # $0 node -v"
  echo -e " \e[32mrelease\e[0m    Start tezedge node in release mode. You can specify additional arguments here."
  echo    "            Release mode means that the node binary is compiled with '--release' flag."
  echo    "              # $0 release -v"
  echo -e " \e[32mdocker\e[0m     Run tezedge node as a docker container. It is possible to specify additional commandline arguments."
  echo    "              # $0 docker -t 4 -T 12"
  echo    "              # $0 docker --help"
  echo -e " \e[32mfuzz\e[0m       Start tezedge node in fuzz mode. You can specify additional arguments here."
  echo    "            Fuzz mode means that the node binary is compiled with most debug options but enabling optimizations."
}

configure_env_variables() {
  options=$(getopt --longoptions debug,release,fuzz,addrsanitizer --options "" --name $0 -- "$@")
  eval set -- "$options"
  while true ; do
    case $1 in
      --debug)
        export PROFILE="debug"
        export CARGO_PROFILE_ARG=""
        shift
        ;;
      --release)
        export PROFILE="release"
        export CARGO_PROFILE_ARG="--release"
        shift
        ;;
      --fuzz)
        export RUSTFLAGS="-Zprofile -C force-frame-pointers --cfg dyncov"
        export PROFILE="fuzz"
        export CARGO_PROFILE_ARG="-Z unstable-options --profile=fuzz"
        shift
        ;;
      --addrsanitizer)
        export RUSTFLAGS="-Z sanitizer=address"
        export CARGO_BUILD_TARGET="x86_64-unknown-linux-gnu"
        export PROFILE="debug"
        export CARGO_PROFILE_ARG=""
        shift
        ;;
      --)
        shift
        break
        ;;
      *)
        echo "Invalid function argument"
        exit 1
        ;;
    esac
  done
}

print_configuration() {
  echo -ne "\033[1;37mRunning Tezedge node in ${PROFILE^^} mode"
  if [ -n "$CARGO_BUILD_TARGET" ]; then
      echo -ne "($CARGO_BUILD_TARGET)"
  fi

  if [ -n "$RUSTFLAGS" ]; then
      echo -ne " extra compilation flags : '$RUSTFLAGS'"
  fi
  echo -ne "\e[0m\n"
}

build_all() {
  export SODIUM_USE_PKG_CONFIG=1
  cargo build $CARGO_PROFILE_ARG
}

run_node() {

  # Default light-node commandline arguments:
  #
  # -d | --tezos-data-dir
  TEZOS_DIR=/tmp/tezedge/tezos-node
  # -c | --config-file-path
  CONFIG_FILE=./light_node/etc/tezedge/tezedge.config
  # -B | --bootstrap-db-path
  BOOTSTRAP_DIR=/tmp/tezedge/light-node
  # -i | --identity
  IDENTITY_FILE=./light_node/etc/tezedge/identity.json
  # rest of the commandline arguments will end up here
  args=()

  shift # shift past <MODE>

  # The following loop is used to replace default light-node commandline parameters.
  # It also allows to specify additional commandline parameters.
  # Supports '--arg=val' and '--arg val' syntax of the commandline arguments.
  while [ "$#" -gt 0 ]; do
    case $1 in
      --tezos-data-dir=*)
        TEZOS_DIR="${1#*=}"
        shift
        ;;
      --tezos-data-dir)
        shift
        TEZOS_DIR="$1"
        shift
        ;;
      --config-file=*)
        CONFIG_FILE="${1#*=}"
        shift
        ;;
      --config-file)
        shift
        CONFIG_FILE="$1"
        shift
        ;;
      --bootstrap-db-path=*)
        BOOTSTRAP_DIR="${1#*=}"
        shift
        ;;
      --bootstrap-db-path)
        shift
        BOOTSTRAP_DIR="$1"
        shift
        ;;
      *)
        args+=("$1")
        shift
        ;;
    esac
  done


  # cleanup data directory
  if [ -z "$KEEP_DATA" ]; then
    rm -rf $TEZOS_DIR $BOOTSTRAP_DIR || true
    mkdir -p $TEZOS_DIR $BOOTSTRAP_DIR
  fi

  # protocol_runner needs 'libtezos.so' to run
  export LD_LIBRARY_PATH="${BASH_SOURCE%/*}/tezos/sys/lib_tezos/artifacts:${BASH_SOURCE%/*}/target/$PROFILE"
  # start node
  if [ -z "$CARGO_BUILD_TARGET" ]; then
    PROTOCOL_RUNNER_BINARY=./target/$PROFILE/protocol-runner
  else
    PROTOCOL_RUNNER_BINARY=./target/$CARGO_BUILD_TARGET/$PROFILE/protocol-runner
  fi

  cargo run $CARGO_PROFILE_ARG --bin light-node -- \
            --config-file "$CONFIG_FILE" \
            --tezos-data-dir "$TEZOS_DIR" \
            --identity-file "$IDENTITY_FILE" \
            --bootstrap-db-path "$BOOTSTRAP_DIR" \
            --protocol-runner $PROTOCOL_RUNNER_BINARY "${args[@]}"
}

run_docker() {
  shift # shift past <MODE>

  # build docker
  docker build -t tezedge-run -f ./docker/run/Dockerfile .
  # run docker
  docker run -i -p 9732:9732 -p 18732:18732 -p 4927:4927 -t tezedge-run "$@"
}

run_sandbox() {
  # Default light-node commandline arguments:
  #
  # -d | --tezos-data-dir
  TEZOS_DIR=/tmp/tezedge/tezos-node
  # -B | --bootstrap-db-path
  BOOTSTRAP_DIR=/tmp/tezedge/light-node
  # -n | --network
  NETWORK=sandbox

  # protocol_runner needs 'libtezos.so' to run
  export LD_LIBRARY_PATH="${BASH_SOURCE%/*}/tezos/sys/lib_tezos/artifacts:${BASH_SOURCE%/*}/target/$PROFILE"

  export TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER="Y"

  cargo run $CARGO_PROFILE_ARG --bin sandbox -- \
            --log-level "info" \
            --sandbox-rpc-port "3030" \
            --light-node-path "./target/$PROFILE/light-node" \
            --protocol-runner-path "./target/$PROFILE/protocol-runner" \
            --tezos-client-path "./sandbox/artifacts/tezos-client" "${args[@]}"
}

case $1 in

  node)
    warn_if_not_using_recommended_rust
    configure_env_variables --debug
    print_configuration
    build_all
    run_node "$@"
    ;;

  node-saddr)
    warn_if_not_using_recommended_rust
    configure_env_variables --debug --addrsanitizer
    print_configuration
    build_all
    run_node "$@"
    ;;

  release)
    warn_if_not_using_recommended_rust
    configure_env_variables --release
    print_configuration
    build_all
    run_node "$@"
    ;;

  fuzz)
    rm -f ./target/fuzz/deps/*.gcda
    warn_if_not_using_recommended_rust
    configure_env_variables --fuzz
    print_configuration
    build_all
    run_node "$@"
    ;;

  sandbox)
    warn_if_not_using_recommended_rust
    configure_env_variables --release
    print_configuration
    build_all
    run_sandbox "$@"
    ;;

  docker)
    echo -ne "\033[1;37mRunning Tezedge node in docker\e[0m\n"
    run_docker "$@"
    ;;

  -h|--help)
    help
    ;;

  *)
    echo "run '$0 --help' to get more info"
    ;;

esac

