# Sandbox launcher

Launcher that enables to start and stop a light node via RPCs. 

Quick start (running in sandbox mode)
-----------


### **1. run the provided run.sh script in sandbox mode**
Runs the launcher
```
./run.sh sandbox
```

### **2. call the start RPC**

Start a tezedge light-node with the provided arguments in request body.

```
curl --location --request POST 'http://127.0.0.1:3030/start' \
--header 'Content-Type: application/json' \
--data-raw '{
    "identity_expected_pow": 0,
    "disable_bootstrap_lookup": "",
    "network": "sandbox",
    "peer_thresh_low": 1,
    "peer_thresh_high": 1,
    "sandbox_patch_context_json_file": "/tmp/sandbox/sandbox-patch-context.json",
    "protocol_runner": "/tmp/sandbox/protocol-runner",
    "tezos_data_dir": "/tmp/tezedge_developer/tezos-node",
    "identity_file": "/tmp/sandbox/light-node/identity.json",
    "bootstrap_db_path": "bootstrap_db",
    "db_cfg_max_threads": "4",
    "log_format": "simple",
    "log_level": "info",
    "ocaml_log_enabled": false,
    "p2p_port": 9732,
    "rpc_port": 18732,
    "monitor_port": 4040,
    "websocket_address": "0.0.0.0:4927",
    "ffi_calls_gc_threshold": 50,
    "ffi_pool_max_connections": 10,
    "ffi_pool_connection_timeout_in_secs": 60,
    "ffi_pool_max_lifetime_in_secs": 21600,
    "ffi_pool_idle_timeout_in_secs": 1800,
    "store_context_actions": false,
    "tokio_threads": 0,
    "enable_testchain": false
}'
```

### **3. call the init_client RPC**

Initializes the tezos-client with the harcoded accounts

```
curl --location --request GET 'http://127.0.0.1:3030/init_client'
```

### **4. call the activate_protocol RPC**

Activates a protocol with the provided parameters.

Note that _timestamp_ field contains the current time in RFC3339 format.
The _protocol\_hash_ field contains the protocol hash of the protocol we want to activate.

```
curl --location --request POST 'http://127.0.0.1:3030/activate_protocol' \
--header 'Content-Type: application/json' \
--data-raw '{
  "timestamp": "2020-06-24T08:02:48Z",
  "protocol_hash": "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb",
  "protocol_parameters": {
    "preserved_cycles": 2,
    "blocks_per_cycle": 8,
    "blocks_per_commitment": 4,
    "blocks_per_roll_snapshot": 4,
    "blocks_per_voting_period": 64,
    "time_between_blocks": [
      "1",
      "0"
    ],
    "endorsers_per_block": 32,
    "hard_gas_limit_per_operation": "1040000",
    "hard_gas_limit_per_block": "10400000",
    "proof_of_work_threshold": "-1",
    "tokens_per_roll": "8000000000",
    "michelson_maximum_type_size": 1000,
    "seed_nonce_revelation_tip": "125000",
    "origination_size": 257,
    "block_security_deposit": "512000000",
    "endorsement_security_deposit": "64000000",
    "baking_reward_per_endorsement": [
      "1250000",
      "187500"
    ],
    "endorsement_reward": [
      "1250000",
      "833333"
    ],
    "cost_per_byte": "50",
    "hard_storage_limit_per_operation": "60000",
    "test_chain_duration": "1966080",
    "quorum_min": 2000,
    "quorum_max": 7000,
    "min_proposal_quorum": 500,
    "initial_endorsers": 1,
    "delay_per_missing_endorsement": "1"
  }
}'
```

You can verify the activated protocol parameters by calling the node's constanta RPC.

```
curl --location --request GET 'http://116.202.128.230:18732/chains/main/blocks/head/context/constants'
```

### **5. call the bake RPC**

Bake a block using the bootstrap1 account 

```
curl --location --request GET 'http://127.0.0.1:3030/bake'
```

### **6. call the stop RPC**

Stopping the node. Also cleans the nodes DB and the the tezos-client directory. 

```
curl --location --request GET 'http://127.0.0.1:3030/stop'
```

