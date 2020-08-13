# RPC

This project contains implementation of Tezos RPC endpoints and Tezedge development endpoints.

#### Integration test
Tezedge CI runs big integration test for RPC's on Drone server, which verifies compatibility Tezedge RPC's with original Tezos Ocaml RPC'c.
- run test and check headers in interval (FROM_BLOCK_HEADER, TO_BLOCK_HEADER>
- FROM_BLOCK_HEADER / TO_BLOCK_HEADER are passed as environment variables
- NODE_RPC_CONTEXT_ROOT_1 (e.g.: http://ocaml-node-run:8732) - env variable where is node1 running
- NODE_RPC_CONTEXT_ROOT_2 (e.g.: http://tezedge-node-run:18732) - env variable where is node2 running
- TODO: added more env vars, add desc. here
```
NODE_RPC_CONTEXT_ROOT_1=http://ocaml-node-run:8732 \
NODE_RPC_CONTEXT_ROOT_2=http://tezedge-node-run:18732 \
FROM_BLOCK_HEADER=1 \
TO_BLOCK_HEADER=5000 \
cargo test --verbose -- --nocapture --ignored test_rpc_compare
```