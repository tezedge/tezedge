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

All successfully started sandbox nodes can be accessed on host/ip `localhost`.
For example, node's RPCs: `http://localhost:18732/version`

```
curl --location --request POST 'http://localhost:3030/start' \
--header 'Content-Type: application/json' \
--data-raw '{
    "identity_expected_pow": 0,
    "disable_bootstrap_lookup": "",
    "network": "sandbox",
    "peer_thresh_low": 1,
    "peer_thresh_high": 1,
    "sandbox_patch_context_json": {
        "genesis_pubkey": "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2"
    },
    "tezos_data_dir": "/tmp/tezedge/tezos-node",
    "identity_file": "/tmp/tezedge/identity.json",
    "bootstrap_db_path": "/tmp/tezedge/light-node",
    "db_cfg_max_threads": "4",
    "log_format": "simple",
    "log_level": "info",
    "ocaml_log_enabled": false,
    "p2p_port": 9732,
    "rpc_port": 18732,
    "websocket_address": "0.0.0.0:4927",
    "ffi_pool_max_connections": 10,
    "ffi_pool_connection_timeout_in_secs": 60,
    "ffi_pool_max_lifetime_in_secs": 21600,
    "ffi_pool_idle_timeout_in_secs": 1800,
    "ffi_trpap_pool_max_connections": 10,
    "ffi_trpap_pool_connection_timeout_in_secs": 60,
    "ffi_trpap_pool_max_lifetime_in_secs": 21600,
    "ffi_trpap_pool_idle_timeout_in_secs": 1800,
    "ffi_twcap_pool_max_connections": 10,
    "ffi_twcap_pool_connection_timeout_in_secs": 60,
    "ffi_twcap_pool_max_lifetime_in_secs": 21600,
    "ffi_twcap_pool_idle_timeout_in_secs": 1800,
    "store_context_actions": false,
    "tokio_threads": 0,
    "enable_testchain": false
  }'
```

### **3. call the init_client RPC**

Initializes the tezos-client with the provided accounts. In this example, we provide 2 accounts.

```
curl --location --request POST 'http://localhost:3030/init_client' \
--header 'Content-Type: application/json' \
--data-raw '[
    {
        "alias": "bootstrap1",
        "secret_key": "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
        "public_key": "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        "public_key_hash": "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx",
        "initial_balance": "4000000000000"
    },
    {
        "alias": "bootstrap2",
        "secret_key": "edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo",
        "public_key": "edpktzNbDAUjUk697W7gYg2CRuBQjyPxbEg8dLccYYwKSKvkPvjtV9",
        "public_key_hash": "tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN",
        "initial_balance": "4000000000000"
    }
]'
```

### **4. call the activate_protocol RPC**

Activates a protocol with the provided parameters.

Note that _timestamp_ field contains the current time in RFC3339 format.
The _protocol_hash_ field contains the protocol hash of the protocol we want to activate.

```
curl --location --request POST 'http://localhost:3030/activate_protocol' \
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
curl --location --request GET 'http://localhost:18732/chains/main/blocks/head/context/constants'
```

### **5. call the bake RPC**

Bake a block using the provided account. (The account must me initialized in previous call of the /init_client endpoint)

```
curl --location --request POST 'http://localhost:3030/bake' \
--header 'Content-Type: application/json' \
--data-raw '{
    "alias": "bootstrap1"
}'
```

### **6. call the list of wallets**

Lists initialized wallets.

```
curl --location --request GET 'http://127.0.0.1:3030/wallets'
```

### **7. call the list of running nodes**

Lists all running nodes.

```
curl --location --request GET 'http://127.0.0.1:3030/list_nodes'
```

### **8. call the stop RPC**

Stopping the node. Also cleans the nodes DB and the the tezos-client directory.

```
curl --location --request GET 'http://127.0.0.1:3030/stop'
```
