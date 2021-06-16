# Running baker/endorser on Mainnet

## Table of Contents
* [Run from sources/binaries](#run-from-sourcesbinaries)
   * [Create working directory](#create-working-directory)
   * [Download snapshot data](#download-snapshot-data)
   * [Identity json file](#identity-json-file)
   * [Run TezEdge node](#run-tezedge-node)
   * [Wait for TezEdge node to sync with network](#wait-for-tezedge-node-to-sync-with-network)
   * [Build Tezos Baker/Endorser/Accuser binaries from source](#build-tezos-bakerendorseraccuser-binaries-from-source)
   * [Initialize keys for baker/endorser](#initialize-keys-for-bakerendorser)
   * [Run baker](#run-baker)
   * [Run endorser](#run-endorser)

# Run from sources/binaries

1. #### **Create working directory**

   _Tezos baker requires access to the context directory, which is produced by TezEdge node._
    ```
    mkdir $HOME/data-mainnet
    mkdir $HOME/data-mainnet/client
    ```

2. #### **Download snapshot data**

   _If you want to run node from empty storage, you can aviod this step._

   There are to ways how to download snapshot data:
   a) **http download**
   ```
   cd $HOME/data-mainnet
   wget -m -np -nH -R 'index.html*' http://65.21.165.81:8080/tezedge_data_from_block_0_1516280 -P $HOME/data-mainnet
   ```

   b) **scp download**
   _For user/password please contact [juraj.selep@viablesystems.io](juraj.selep@viablesystems.io)_
   ```
   cd $HOME/data-mainnet
   scp -r <user>@65.21.165.81:/home/dev/tezedge-data/tezedge_data_from_block_0_1516280 $HOME/data-mainnet
   passwd: <password>
   ```

   _Once downloaded, run this command:_
   ```
   cd $HOME/data-mainnet
   cp -R ./tezedge_data_from_block_0_1516280/_data/* $HOME/data-mainnet
   ```

3. #### **Identity json file**

   a) If you already have `identity.json`, just copy it here
   ```
   cp <your-identity.json> $HOME/data-mainnet/identity.json
   ```

   b) If you dont have, dont worry, TezEdge node will generate one here:
   ```
   $HOME/data-mainnet/identity.json
   ```

4. #### **Run TezEdge node**

   At first you need to:
   - check [supported OS](../../README.md#supported-os-distributions)
   - install [prerequisites](../../README.md#prerequisites-installation)
   - build [TezEdge from sources](../../README.md#build-from-source-code)
   - check [how to run it](../../README.md#how-to-run)

   Once everything works, you can continue to run node.

   _Note: This cmd runs from the main TezEdge git sources directory - see the relative './' paths_
   ```
   LD_LIBRARY_PATH=./tezos/sys/lib_tezos/artifacts ./target/release/light-node \
    --network "mainnet" \
    --identity-file "$HOME/data-mainnet/identity.json" \
    --identity-expected-pow 26.0 \
    --tezos-data-dir "$HOME/data-mainnet" \
    --bootstrap-db-path "$HOME/data-mainnet/bootstrap_db" \
    --peer-thresh-low 30 --peer-thresh-high 45 \
    --protocol-runner "./target/release/protocol-runner" \
    --init-sapling-spend-params-file "./tezos/sys/lib_tezos/artifacts/sapling-spend.params" \
    --init-sapling-output-params-file "./tezos/sys/lib_tezos/artifacts/sapling-output.params" \
    --p2p-port 19732 --rpc-port 18732 \
    --websocket-address 0.0.0.0:17732 \
    --tokio-threads 0 \
    --ocaml-log-enabled false \
    --one-context \
    --actions-store-backend none \
    --log terminal \
    --log-level info \
    --log-format simple
   ```

   *You should see output:*
   ```
   Jun 16 13:01:24.804 INFO Configured network ["mainnet"] -> TEZOS_MAINNET
   Jun 16 13:01:24.804 INFO Checking zcash-params for sapling... (1/5)
   Jun 16 13:01:24.804 INFO Creating new zcash-params dir, dir: "/home/dev/.zcash-params"
   Jun 16 13:01:24.804 INFO Using configured init files for zcash-params, output_path: "./tezos/sys/lib_tezos/artifacts/sapling-output.params", spend_path: "./tezos/sys/lib_tezos/artifacts/sapling-spend.params"
   Jun 16 13:01:24.834 INFO Sapling zcash-params files were created, output_path: "/home/dev/.zcash-params/sapling-output.params", spend_path: "/home/dev/.zcash-params/sapling-spend.params", dir: "/home/dev/.zcash-params"
   Jun 16 13:01:24.834 INFO Loading identity... (2/5)
   ...
   ```

   _Note1: Now TezEdge node is running._

   _Note2: Recommended to run node with nohup_

   ```
   LD_LIBRARY_PATH=./tezos/sys/lib_tezos/artifacts nohup ./target/release/light-node \
    --network "mainnet" \
    --identity-file "$HOME/data-mainnet/identity.json" \
    --identity-expected-pow 26.0 \
    --tezos-data-dir "$HOME/data-mainnet" \
    --bootstrap-db-path "$HOME/data-mainnet/bootstrap_db" \
    --peer-thresh-low 30 --peer-thresh-high 45 \
    --protocol-runner "./target/release/protocol-runner" \
    --init-sapling-spend-params-file "./tezos/sys/lib_tezos/artifacts/sapling-spend.params" \
    --init-sapling-output-params-file "./tezos/sys/lib_tezos/artifacts/sapling-output.params" \
    --p2p-port 19732 --rpc-port 18732 \
    --websocket-address 0.0.0.0:17732 \
    --tokio-threads 0 \
    --ocaml-log-enabled false \
    --one-context \
    --actions-store-backend none \
    --log terminal \
    --log-level info \
    --log-format simple &> $HOME/data-mainnet/nohup-node.out &
   ```

   You can check logs:
   ```
   tail -f $HOME/data-mainnet/nohup-node.out
   ```

5. #### **Wait for TezEdge node to sync with network**

   _Check RPC for currrent head to reach the Mainnet:_
   ```
   curl http:://localhost:18732/chains/main/blocks/head/header
   ```

   Or check logs and see `local_level` to reach the Mainnet:
   ```
   dev@test-node:~/tezedge$ cat $HOME/data-mainnet/nohup-node.out | grep Head
   ...
   Jun 16 13:14:19.132 INFO Head info, remote_fitness: 01::00000000000d2965, remote_level: 1517925, remote: BLas1fJWijecs5oaD7Uk8m44bHRHA36WhK8w31j98NGaMGHMBzY, local_fitness: 01::00000000000d23f5, local_level: 1516533, local: BLfcJotjcgkzbcJT2ZNSoziGxds4YjBNfruAo5UtRdVVSMshdYu
   ...
   ```

6. #### **Build Tezos Baker/Endorser/Accuser binaries from source**

   Please, see [https://tezos.gitlab.io/introduction/howtoget.html#building-from-sources-via-opam](https://tezos.gitlab.io/introduction/howtoget.html#building-from-sources-via-opam)

   After successfull compilation, you should see this binaries in Tezos source directory:
   ```
   tezos-baker-009-PsFLoren
   tezos-baker-010-PtGRANAD
   tezos-client
   tezos-endorser-009-PsFLoren
   tezos-endorser-010-PtGRANAD
   ```

7. #### **Initialize keys for baker/endorser**

   See https://tezos.gitlab.io/user/key-management.html

   _It is recommended to have different accounts for baker/endorser/accuser._

   _Note: All cmd runs from the main Tezos git sources directory_

   a) If you already have existing accounts for baking/endorsing, you just need to import keys:
   ```
   ./tezos-client \
      --endpoint "http://localhost:18732" \
      --base-dir "$HOME/data-mainnet/client" \
      --log-requests \
       import secret key <baker_alias> <baker_alias_secret_key>
   ```
   ```
   ./tezos-client \
      --endpoint "http://localhost:18732" \
      --base-dir "$HOME/data-mainnet/client" \
      --log-requests \
       import secret key <endorser_alias> <endorser_alias_secret_key>
   ```

   b) If you dont have any keys, you need to activate and register accounts as delegate:

   _TezEdge node have to be synced already._
   ```
   ./tezos-client \
      --endpoint "http://localhost:18732" \
      --base-dir "$HOME/data-mainnet/client" \
      --log-requests \
       activate account <baker_alias> with "<key/ledger/faucet>"
   ```

   ```
   ./tezos-client \
      --endpoint "http://localhost:18732" \
      --base-dir "$HOME/data-mainnet/client" \
      --log-requests \
       activate account <endorser_alias> with "<key/ledger/faucet>"
   ```

   *Once operations are baked, you should see output:*
   ```
   ...
   Account baker (tz1XXXXXX) activated with ꜩ76351.572618.
   Account endorser (tz1XXXXXX) activated with ꜩ76351.572618.
   ...
   ```

   And then register as delegate:

   ```
   ./tezos-client \
      --endpoint "http://localhost:18732" \
      --base-dir "$HOME/data-mainnet/client" \
      --log-requests \
       register key <baker_alias> as delegate
   ```

   ```
   ./tezos-client \
      --endpoint "http://localhost:18732" \
      --base-dir "$HOME/data-mainnet/client" \
      --log-requests \
       register key <endorser_alias> as delegate
   ```

   *Once operations are baked, you should see output:*
   ```
   ...
   This revelation was successfully applied
   ...
   This delegation was successfully applied
   ...
   ```

8. #### **Run baker**

   _Note: All cmd runs from the main Tezos git sources directory_
   ```
   ./tezos-baker-009-PsFLoren \
      --endpoint "http://localhost:18732" \
      --base-dir "$HOME/data-mainnet/client" \
      --log-requests \
      run with local node "$HOME/data-mainnet" <baker_alias>
   ```

   *You should see output:*
   ```
   Node is bootstrapped.
   ...
   Baker started.
   ...
   Jun 10 13:11:53.782 - 009-PsFLoren.delegate.baking_forge: no slot found at level 41427 (max_priority = 64)

   ...
   ```

9. #### **Run endorser**
   ```
   ./tezos-endorser-009-PsFLoren \
      --endpoint "http://localhost:18732" \
      --base-dir "$HOME/data-mainnet/client" \
      --log-requests \
      run <endorser_alias>
   ```

   *You should see output:*
   ```
   Node is bootstrapped.
   ...
   Endorser started.
   ...
   ```