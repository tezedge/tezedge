#!/bin/bash

# add rust to path
PATH=/home/appuser/.cargo/bin:$PATH

# light node config
NETWORK="babylonnet"
TEZOS_DIR="/tmp/tezedge/tezos-data"
BOOTSTRAP_DIR="/tmp/tezedge/tezedge-data/"
CONFIG_FILE="./light_node/etc/tezedge/tezedge.config"

# cleanup data directory
rm -rf $BOOTSTRAP_DIR && mkdir $BOOTSTRAP_DIR 
rm -rf $TEZOS_DIR && mkdir $TEZOS_DIR

# protocol_runner needs 'libtezos.so' to run
export LD_LIBRARY_PATH="/home/appuser/tezedge/tezos/interop/lib_tezos/artifacts:/home/appuser/tezedge/target/release"

# start node
cargo run --release --bin light-node -- \
                            --config-file "$CONFIG_FILE" \
                            --tezos-data-dir "$TEZOS_DIR" \
                            --bootstrap-db-path "$BOOTSTRAP_DIR" \
                            --network "$NETWORK" \
                            --ocaml-log-enabled "fasle" \
                            --protocol-runner "./target/release/protocol-runner"