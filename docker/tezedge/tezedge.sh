#!/bin/sh

# delete tezos-data 
# rm -rf /var/tezedge/tezos-data/*

# delete tezedge-data 
# rm -rf /var/tezedge/tezedge-data/*

# run tezedge node 
light-node \
    --network="babylonnet" \
    --protocol-runner="/bin/protocol-runner" \
    --tezos-data-dir="/var/tezedge/tezos-data/" \
    --bootstrap-db-path="/var/tezedge/tezedge-data/" \
    --identity="/var/tezedge/identity.json" 