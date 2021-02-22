#!/bin/bash

LAUNCHER=http://127.0.0.1:3030

start_sandbox_node() {
  # start sendbox node
  curl --location --request POST $LAUNCHER'/start' \
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
      "store_context_actions": false,
      "tokio_threads": 0,
      "enable_testchain": false
    }'

  echo "\nStarting sandbox node..."
  sleep 3


  # init tezos client for sandbox node
  curl --location --request POST $LAUNCHER'/init_client' \
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
      },
      {
          "alias": "bootstrap3",
          "secret_key": "edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ",
          "public_key": "edpkuTXkJDGcFd5nh6VvMz8phXxU3Bi7h6hqgywNFi1vZTfQNnS1RV",
          "public_key_hash": "tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU",
          "initial_balance": "4000000000000"
      },
      {
          "alias": "bootstrap4",
          "secret_key": "edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3",
          "public_key": "edpkuFrRoDSEbJYgxRtLx2ps82UdaYc1WwfS9sE11yhauZt5DgCHbU",
          "public_key_hash": "tz1b7tUupMgCNw2cCLpKTkSD1NZzB5TkP2sv",
          "initial_balance": "4000000000000"
      },
      {
          "alias": "bootstrap5",
          "secret_key": "edsk4QLrcijEffxV31gGdN2HU7UpyJjA8drFoNcmnB28n89YjPNRFm",
          "public_key": "edpkv8EUUH68jmo3f7Um5PezmfGrRF24gnfLpH3sVNwJnV5bVCxL2n",
          "public_key_hash": "tz1ddb9NMYHZi5UzPdzTZMYQQZoMub195zgv",
          "initial_balance": "4000000000000"
      }
  ]'

  sleep 1
  echo "\nTezos client initialized..."

  # init protocol
  curl --location --request POST $LAUNCHER'/activate_protocol' \
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

  sleep 1
  echo "\nProtocol initialized..."
  echo "\nSandbox node is ready..."
}

stop_sandbox_node() {
	curl --location --request GET 'http://127.0.0.1:3030/stop'
}

send_transactions() {
	# get running node
	sandbox_node=$(curl -s --location --request GET  $LAUNCHER'/list_nodes')
	sandbox_node=$(echo $sandbox_node | jq '.[0]')
	sandbox_node_host=$(echo $sandbox_node | jq -r '.ip')
	sandbox_node_port=$(echo $sandbox_node | jq -r '.port')
	echo "Sandbox node: " $sandbox_node_host " port: " $sandbox_node_port

	# read wallets
	wallets=$(curl -s --location --request GET $LAUNCHER'/wallets' | jq -S .)
	# get secret keys
	wallets_json=$(mktemp -t wallets-XXXXXXXXXX.json)
	echo $wallets > $wallets_json

	# init several tezos clients with own data dir
	export TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER="Y"
	tezos_client=$2
	if test -f "$tezos_client"; then
		echo "Tezos_client binary path: '$tezos_client' exist."
	else
	  tezos_client="./sandbox/artifacts/tezos-client"
	  if test -f "$tezos_client"; then
		  echo "Tezos_client binary path: '$tezos_client' exist."
	  else
		  echo "Tezos_client binary path: '$tezos_client' does not exist."
		  exit;
		fi
	fi

	tezos_client_wrappers_0="$tezos_client --base-dir $(mktemp -d -t tcd-XXXXXXXXXX) -A $sandbox_node_host -P $sandbox_node_port"
	tezos_client_wrappers_1="$tezos_client --base-dir $(mktemp -d -t tcd-XXXXXXXXXX) -A $sandbox_node_host -P $sandbox_node_port"
	tezos_client_wrappers_2="$tezos_client --base-dir $(mktemp -d -t tcd-XXXXXXXXXX) -A $sandbox_node_host -P $sandbox_node_port"
	tezos_client_wrappers_3="$tezos_client --base-dir $(mktemp -d -t tcd-XXXXXXXXXX) -A $sandbox_node_host -P $sandbox_node_port"
	tezos_client_wrappers_4="$tezos_client --base-dir $(mktemp -d -t tcd-XXXXXXXXXX) -A $sandbox_node_host -P $sandbox_node_port"


	# initialize tezos-client-binary with wallets
	jq -c '.[]' $wallets_json | while read wallet; do
		alias=$(echo $wallet | jq -r '.alias')
		secret_key=unencrypted:$(echo $wallet | jq -r '.secret_key')
		echo "Initialize wallet: " $alias " - " $secret_key

		$tezos_client_wrappers_0 import secret key $alias $secret_key
		$tezos_client_wrappers_1 import secret key $alias $secret_key
		$tezos_client_wrappers_2 import secret key $alias $secret_key
		$tezos_client_wrappers_3 import secret key $alias $secret_key
		$tezos_client_wrappers_4 import secret key $alias $secret_key
	done

	# send transactions
	wallets_aliases=($(echo $wallets | jq -r '.[].alias' | tr " " "\n"))
	wallets_aliases_count=${#wallets_aliases[@]}
	echo $wallets_aliases_count

	for i in {0..20}; do {
		wallet1=$(( $RANDOM % $wallets_aliases_count))
		wallet2=$(( $RANDOM % $wallets_aliases_count))
		wallet1=${wallets_aliases[$wallet1]}
		wallet2=${wallets_aliases[$wallet2]}
		burn_cap=$(( $RANDOM % 10 + 1))
		transfer=$(( $RANDOM % 1000 + 1))

		client=$(( $RANDOM % 4))

		case $client in
			0) $tezos_client_wrappers_0 transfer $transfer from $wallet1 to $wallet2 --burn-cap $burn_cap &;;
			1) $tezos_client_wrappers_0 transfer $transfer from $wallet1 to $wallet2 --burn-cap $burn_cap &;;
			2) $tezos_client_wrappers_0 transfer $transfer from $wallet1 to $wallet2 --burn-cap $burn_cap &;;
			3) $tezos_client_wrappers_0 transfer $transfer from $wallet1 to $wallet2 --burn-cap $burn_cap &;;
			4) $tezos_client_wrappers_0 transfer $transfer from $wallet1 to $wallet2 --burn-cap $burn_cap &;;
		esac
	} done
}

case $1 in

  start_sandbox_node)
	start_sandbox_node
   ;;
  stop_sandbox_node)
	stop_sandbox_node
   ;;
  send_transactions)
	send_transactions "$@"
   ;;

  *)
    echo "Invalid command '$1'"
    ;;

esac

# ./run.sh sandbox
# ./test.sh start_sandbox_node
# ps -ef | grep runner
# ./test.sh send_transactions /home/dev/tezos/tezos-client
# ps -ef | grep runner
# curl http://trace.dev.tezedge.com:18732/chains/main/mempool/pending_operations
# ./test.sh stop_sandbox_node
