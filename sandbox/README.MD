# Sandbox launcher

Launcher that enables to start and stop a light node via RPCs.

Quick start (running in sandbox mode)
-----------


### **1. run  sandbox mode**

There are several ways how to run sandbox launcher:

1. Runs the launcher directly from source code
```
./run.sh sandbox
```

2. Use docker-compose

[Prearranged sandbox docker-compose ](../README.md#sandbox---sandbox-launcher--light-node--tezedge-explorer)


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
    "synchronization_thresh": "0",
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

#### Activate 009 Florence
```
curl --location --request POST 'http://localhost:3030/activate_protocol' \
--header 'Content-Type: application/json' \
--data-raw '{
  "timestamp": "2021-06-08T08:02:48Z",
  "protocol_hash": "PsFLorenaUUuikDWvMDr6fGBRG8kt3e3D3fHoXK1j1BFRxeSH4i",
  "protocol_parameters": {
      "bootstrap_accounts": [
        [
          "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
          "4000000000000"
        ],
        [
          "edpktzNbDAUjUk697W7gYg2CRuBQjyPxbEg8dLccYYwKSKvkPvjtV9",
          "4000000000000"
        ],
        [
          "edpkuTXkJDGcFd5nh6VvMz8phXxU3Bi7h6hqgywNFi1vZTfQNnS1RV",
          "4000000000000"
        ],
        [
          "edpkuFrRoDSEbJYgxRtLx2ps82UdaYc1WwfS9sE11yhauZt5DgCHbU",
          "4000000000000"
        ],
        [
          "edpkv8EUUH68jmo3f7Um5PezmfGrRF24gnfLpH3sVNwJnV5bVCxL2n",
          "4000000000000"
        ]
      ],
      "commitments": [
        [
          "btz1bRL4X5BWo2Fj4EsBdUwexXqgTf75uf1qa",
          "23932454669343"
        ],
        [
          "btz1SxjV1syBgftgKy721czKi3arVkVwYUFSv",
          "72954577464032"
        ],
        [
          "btz1LtoNCjiW23txBTenALaf5H6NKF1L3c1gw",
          "217487035428348"
        ],
        [
          "btz1SUd3mMhEBcWudrn8u361MVAec4WYCcFoy",
          "4092742372031"
        ],
        [
          "btz1MvBXf4orko1tsGmzkjLbpYSgnwUjEe81r",
          "17590039016550"
        ],
        [
          "btz1LoDZ3zsjgG3k3cqTpUMc9bsXbchu9qMXT",
          "26322312350555"
        ],
        [
          "btz1RMfq456hFV5AeDiZcQuZhoMv2dMpb9hpP",
          "244951387881443"
        ],
        [
          "btz1Y9roTh4A7PsMBkp8AgdVFrqUDNaBE59y1",
          "80065050465525"
        ],
        [
          "btz1Q1N2ePwhVw5ED3aaRVek6EBzYs1GDkSVD",
          "3569618927693"
        ],
        [
          "btz1VFFVsVMYHd5WfaDTAt92BeQYGK8Ri4eLy",
          "9034781424478"
        ]
      ],
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
      "cost_per_byte": "250",
      "hard_storage_limit_per_operation": "60000",
      "test_chain_duration": "0",
      "quorum_min": 2000,
      "quorum_max": 7000,
      "min_proposal_quorum": 500,
      "initial_endorsers": 1,
      "delay_per_missing_endorsement": "1"
    }
}'
```

#### Activate 010 Granada

```
curl --location --request POST 'http://localhost:3030/activate_protocol' \
--header 'Content-Type: application/json' \
--data-raw '{
  "timestamp": "2021-06-08T08:02:48Z",
  "protocol_hash": "PtGRANADsDU8R9daYKAgWnQYAJ64omN1o3KMGVCykShA97vQbvV",
  "protocol_parameters": {
      "bootstrap_accounts": [
        [
          "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
          "4000000000000"
        ],
        [
          "edpktzNbDAUjUk697W7gYg2CRuBQjyPxbEg8dLccYYwKSKvkPvjtV9",
          "4000000000000"
        ],
        [
          "edpkuTXkJDGcFd5nh6VvMz8phXxU3Bi7h6hqgywNFi1vZTfQNnS1RV",
          "4000000000000"
        ],
        [
          "edpkuFrRoDSEbJYgxRtLx2ps82UdaYc1WwfS9sE11yhauZt5DgCHbU",
          "4000000000000"
        ],
        [
          "edpkv8EUUH68jmo3f7Um5PezmfGrRF24gnfLpH3sVNwJnV5bVCxL2n",
          "4000000000000"
        ]
      ],
      "commitments": [
        [
          "btz1bRL4X5BWo2Fj4EsBdUwexXqgTf75uf1qa",
          "23932454669343"
        ],
        [
          "btz1SxjV1syBgftgKy721czKi3arVkVwYUFSv",
          "72954577464032"
        ],
        [
          "btz1LtoNCjiW23txBTenALaf5H6NKF1L3c1gw",
          "217487035428348"
        ],
        [
          "btz1SUd3mMhEBcWudrn8u361MVAec4WYCcFoy",
          "4092742372031"
        ],
        [
          "btz1MvBXf4orko1tsGmzkjLbpYSgnwUjEe81r",
          "17590039016550"
        ],
        [
          "btz1LoDZ3zsjgG3k3cqTpUMc9bsXbchu9qMXT",
          "26322312350555"
        ],
        [
          "btz1RMfq456hFV5AeDiZcQuZhoMv2dMpb9hpP",
          "244951387881443"
        ],
        [
          "btz1Y9roTh4A7PsMBkp8AgdVFrqUDNaBE59y1",
          "80065050465525"
        ],
        [
          "btz1Q1N2ePwhVw5ED3aaRVek6EBzYs1GDkSVD",
          "3569618927693"
        ],
        [
          "btz1VFFVsVMYHd5WfaDTAt92BeQYGK8Ri4eLy",
          "9034781424478"
        ]
      ],
      "preserved_cycles": 2,
      "blocks_per_cycle": 8,
      "blocks_per_commitment": 4,
      "blocks_per_roll_snapshot": 4,
      "blocks_per_voting_period": 64,
      "time_between_blocks": [
        "1",
        "0"
      ],
      "endorsers_per_block": 256,
      "hard_gas_limit_per_operation": "1040000",
      "hard_gas_limit_per_block": "5200000",
      "proof_of_work_threshold": "-1",
      "tokens_per_roll": "8000000000",
      "michelson_maximum_type_size": 1000,
      "seed_nonce_revelation_tip": "125000",
      "origination_size": 257,
      "block_security_deposit": "640000000",
      "endorsement_security_deposit": "2500000",
      "baking_reward_per_endorsement": [
        "78125",
        "11719"
      ],
      "endorsement_reward": [
        "78125",
        "52083"
      ],
      "cost_per_byte": "250",
      "hard_storage_limit_per_operation": "60000",
      "quorum_min": 2000,
      "quorum_max": 7000,
      "min_proposal_quorum": 500,
      "initial_endorsers": 1,
      "delay_per_missing_endorsement": "1",
      "minimal_block_delay": "1",
      "liquidity_baking_subsidy": "2500000",
      "liquidity_baking_sunset_level": 4096,
      "liquidity_baking_escape_ema_threshold": 1000000
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
