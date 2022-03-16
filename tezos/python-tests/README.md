# Tezos python test framework

Tezos python test framework run with the tezedge node

## Mixed node tests

By default tests will be executed using `light-node`, but it is possible to mix between `tezos-node` and `light-node` when running the tests.

This is controlled by the `TEZOS_NODE_SEQUENCE` environment variable. It should be a sequence of `T` and `O` letters, specifying the order in which the nodes will be picked. The specified sequence will be cycled to make it infinite.

Examples:

- `T` = launch only `light-node`
- `O` = launch only `tezos-node`
- `TO` = launch `light-node` followed by `tezos-node`, and repeat
- `OOT` = launch two `tezos-node` followed by one `light-node` and repeat

## Tests (THIS TABLE IS OUTDATED)

|             Test              |Tezedge compatibility          |local  | drone  |               Description                                               | Modifications                            |       Comment
|-------------------------------|-------------------------------|-------|--------|-------------------------------------------------------------------------|------------------------------------------|-------------------------------------------------|
| test_basic                    |           Partiall            |failing|failing |basic node functionality and interactions with tezos-client              | None                                     |  RPC error handling and returned errors         |
| test_baker_endorser           |           Full                |passing|passing |baker and endorser daemon interaction, baking blocks, syncing afterwards | None                                     | None  |
| test_multinode                |           Full                |passing|passing |injection - protocol alpha, inclusion of transfer, endorsement operations| None                                     | None                                         |
| test_many_bakers              |           Full                |passing|passing |Run 5 bakers and num nodes, wait and check logs                          | None                                     | None|
| test_many_nodes               |           Full                |passing|passing |Run many nodes, wait a while, run more nodes, check logs                 | None                                     | None|
| test_mempool                  |           Full                |passing|passing |inject ops, check mempool before and after, bake blocks                  | None                                     | None                                         |
| test_fork                     |           Full                |passing|passing |inject ops, check mempool before and after, bake blocks                  | None                                     | None          |
| test_voting                   |           Full                |failing|failing |Test voting protocol with manual baking, 4 blocks per voting period      | None                                     | Implement protocol injection |
| test_tls                      |           ?                   |-      | -      |test client interaction (test_bootstrapped) with node tls                | None                                     | Implement tls in tezedge                     |
| test_rpc                      |           ?                   |-      | -      |tests all rpcs in tezos node and protocol                                | None                                     | Implement all rpcs in tezedge                |
| test_programs                 |           Full                |passing|passing |convert script - mostly client stuff (no unimplemented node RPCs-invest.)| None                                     | None                                            |
| test_openapi                  |           ?                   |-      | -      |test openapi/swagger generation                                          | None                                     | Implement openapi/swagger                      |
| test_p2p                      |           ?                   |-      | -      |see: https://gitlab.com/tezos/tezos/-/blob/v8-release/tests_python/tests/test_p2p.py| None                          | Implement rpcs for p2p (/network/peers)        |
| test_multinode_snapshot       |           ?                   |-      |-       |creating snapshots                                                       | None                                     | Implement snapshot and reconstruct             |
| test_multinode_storage_reconstruction|              ?         |-      |-       |reconstruction from snapshot                                             | None                                     | Implement snapshot and reconstruct             |
| test_injection                |           ?                   |-      | -      |testing protocol injection and activation of injected protocol           | None                                     | Implement protocol injection                   |
| test_double_endorsement       |           Full                |passing|passing |testing double endorsement op and accusation                             | None                                     | None           |
| test_contract                 |           266/286             |failing| -      |testing various contract operations (including smart contracts)          | None                                     | More investigation neeed, mostly error handling/message stuff|
| test_contract_opcodes         |           270/282             |failing| -      |individual opcodes that do not require originations (including smart contracts)| None                                     | More investigation neeed, mostly error handling/message stuff|
| test_contract_baker           |           Full                |passing|passing |Test a simple contract origination and call                              | None                                     | None          |
| test_contract_annotations     |           Full                |passing|passing |Tests of Michelson annotations. - mostly client stuff                    | None                                     | None         |
| test_baker_endorser_mb        |           Full                |-      | -      |test two separate git branch of binaries                                 | None                                     | Later use                                      |
