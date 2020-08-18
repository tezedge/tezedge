# Launcher

Launcher that enables to start and stop a light node via RPCs. 

Quick start (running in sandbox mode)
-----------


**1. run the provided run.sh script in sandbox mode**

```
./run.sh sandbox
```

**2. call the start RPC**

```
curl --location --request POST 'http://127.0.0.1:3030/start' \
--header 'Content-Type: application/json' \
--data-raw '{
    "config_file": "./light_node/etc/tezedge/tezedge.config",
    "identity_expected_pow": 0,
    "disable_bootstrap_lookup": true,
    "network": "sandbox",
    "peer_thresh_low": 1,
    "peer_thresh_high": 1,
    "sandbox_patch_context_json_file": "./light_node/etc/tezedge_sandbox/sandbox-patch-context.json",
    "protocol_runner": "./target/release/protocol-runner"
}'
```

**3. call the stop RPC**

```
curl --location --request GET 'http://127.0.0.1:3030/stop'
```

