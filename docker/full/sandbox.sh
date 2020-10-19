#!/bin/bash

# add rust to path
export HOME=/home/appuser
export PATH=/home/appuser/.cargo/bin:$PATH

# protocol_runner needs 'libtezos.so' to run
export LD_LIBRARY_PATH="/home/appuser/tezedge/tezos/interop/lib_tezos/artifacts:/home/appuser/tezedge/target/release"
export TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER="Y"

./target/release/sandbox    --log-level "info" \
                            --sandbox-rpc-port "3030" \
                            --light-node-path "./target/release/light-node" \
                            --protocol-runner-path "./target/release/protocol-runner" \
                            --tezos-client-path "./sandbox/artifacts/tezos-client"