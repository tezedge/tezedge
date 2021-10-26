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

1. #### **Create a working directory**

   _The Tezos baker requires access to the context directory, which is produced by TezEdge node._
    ```
    mkdir $HOME/data-mainnet
    mkdir $HOME/data-mainnet/client
    ```

2. #### **Download snapshot data**

   _If you want to run node from empty storage, you can skip this step._
   At first, you need to check, which (latest) snapshot to download, open a browser here: http://65.21.165.81/

   There are two ways how to download the snapshot data:

   a) **http download**
   ```
   cd $HOME/data-mainnet
   wget -m -np -nH --cut-dirs=2 -R 'index.html*,identity.json,tezedge.log*' http://65.21.165.81/tezedge_.../_data/
   ```

   b) **scp download**
   _For user/password please contact [jurajselep@viablesystems.io](jurajselep@viablesystems.io)_
   ```
   cd $HOME/data-mainnet
   scp -r <user>@65.21.165.81:/usr/local/etc/tezedge-data/tezedge_.../ $HOME/data-mainnet
   passwd: <password>
   ```

   _Once downloaded, run this command:_
   ```
   cd $HOME/data-mainnet
   cp -R ./tezedge_1784665_2021-10-17T12:04:10.670066045+00:00/_data/* $HOME/data-mainnet
   ```

4. #### **Identity json file**

   a) If you already have the `identity.json` file, just copy it here
   ```
   cp <your-identity.json> $HOME/data-mainnet/identity.json
   ```

   b) If you dont have it, don't worry. The TezEdge node will generate one here:
   ```
   $HOME/data-mainnet/identity.json
   ```

5. #### **Run the TezEdge node**

   First, you need to:
   - check [supported OS](../../README.md#supported-os-distributions)
   - install [prerequisites](../../README.md#prerequisites-installation)
   - build [TezEdge from sources](../../README.md#build-from-source-code)
   - check [how to run it](../../README.md#how-to-run)

   Once everything works, you can continue to launching the node.

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
    --tokio-threads 0 \
    --ocaml-log-enabled false \
    --log terminal \
    --log-level info \
    --log-format simple
   ```

   *You should see this output:*
   ```
   Jun 16 13:01:24.804 INFO Configured network ["mainnet"] -> TEZOS_MAINNET
   Jun 16 13:01:24.804 INFO Checking zcash-params for sapling... (1/5)
   Jun 16 13:01:24.804 INFO Creating new zcash-params dir, dir: "/home/dev/.zcash-params"
   Jun 16 13:01:24.804 INFO Using configured init files for zcash-params, output_path: "./tezos/sys/lib_tezos/artifacts/sapling-output.params", spend_path: "./tezos/sys/lib_tezos/artifacts/sapling-spend.params"
   Jun 16 13:01:24.834 INFO Sapling zcash-params files were created, output_path: "/home/dev/.zcash-params/sapling-output.params", spend_path: "/home/dev/.zcash-params/sapling-spend.params", dir: "/home/dev/.zcash-params"
   Jun 16 13:01:24.834 INFO Loading identity... (2/5)
   ...
   ```

   _Note1: Now the TezEdge node is running._

   _Note2: It is recommended to run node with nohup_

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
    --tokio-threads 0 \
    --ocaml-log-enabled false \
    --log terminal \
    --log-level info \
    --log-format simple &> $HOME/data-mainnet/nohup-node.out &
   ```

   You can check the logs by typing this command:
   ```
   tail -f $HOME/data-mainnet/nohup-node.out
   ```

6. #### **Wait for the TezEdge node to sync with the network**

   _Check RPC for current head to reach the Mainnet:_
   ```
   curl http:://localhost:18732/chains/main/blocks/head/header
   ```

   Or check the logs and see if `local_level` has reached the Mainnet:
   ```
   dev@test-node:~/tezedge$ cat $HOME/data-mainnet/nohup-node.out | grep Head
   ...
   Jun 16 13:14:19.132 INFO Head info, remote_fitness: 01::00000000000d2965, remote_level: 1517925, remote: BLas1fJWijecs5oaD7Uk8m44bHRHA36WhK8w31j98NGaMGHMBzY, local_fitness: 01::00000000000d23f5, local_level: 1516533, local: BLfcJotjcgkzbcJT2ZNSoziGxds4YjBNfruAo5UtRdVVSMshdYu
   ...
   ```

7. #### **Build Tezos Baker/Endorser/Accuser binaries from source**

   Please see [https://tezos.gitlab.io/introduction/howtoget.html#building-from-sources-via-opam](https://tezos.gitlab.io/introduction/howtoget.html#building-from-sources-via-opam)

   After successful compilation, you should see these binaries in Tezos source directory:
   ```
   tezos-baker-009-PsFLoren
   tezos-baker-010-PtGRANAD
   tezos-client
   tezos-endorser-009-PsFLoren
   tezos-endorser-010-PtGRANAD
   ```

8. #### **Initialize keys for baker/endorser**

   See https://tezos.gitlab.io/user/key-management.html

   _It is recommended to have different accounts for baker/endorser/accuser._

   _Note: All cmd run from the main Tezos git sources directory_

   a) If you already have existing accounts for baking/endorsing, you just need to import the keys:
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

   _The TezEdge node has to be already synced._
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

   *Once operations are baked, you should see this output:*
   ```
   ...
   Account baker (tz1XXXXXX) activated with ꜩ76351.572618.
   Account endorser (tz1XXXXXX) activated with ꜩ76351.572618.
   ...
   ```

   And then register as a delegate:

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

   *Once operations are baked, you should see this output:*
   ```
   ...
   This revelation was successfully applied
   ...
   This delegation was successfully applied
   ...
   ```

9. #### **Run baker**

   _Note: All cmd run from the main Tezos git sources directory_
   ```
   ./tezos-baker-009-PsFLoren \
      --endpoint "http://localhost:18732" \
      --base-dir "$HOME/data-mainnet/client" \
      --log-requests \
      run with local node "$HOME/data-mainnet" <baker_alias>
   ```

   *You should see this output:*
   ```
   Node is bootstrapped.
   ...
   Baker started.
   ...
   Jun 10 13:11:53.782 - 009-PsFLoren.delegate.baking_forge: no slot found at level 41427 (max_priority = 64)

   ...
   ```

10. #### **Run endorser**
    ```
    ./tezos-endorser-009-PsFLoren \
       --endpoint "http://localhost:18732" \
       --base-dir "$HOME/data-mainnet/client" \
       --log-requests \
       run <endorser_alias>
    ```

    *You should see this output:*
    ```
    Node is bootstrapped.
    ...
    Endorser started.
    ...
    ```
