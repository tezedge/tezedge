# Runing baker/endorser/accuser on Testnet (Hangzhounet)

## Table of Contents
* [Get free XTZ](#get-free-xtz)
* [Run from sources/binaries](#run-from-sourcesbinaries)
* [Use Ledger](#use-ledger)
* [Remote Signing](#remote-signing)

# Get free XTZ

https://tezos.gitlab.io/introduction/howtouse.html#get-free-tez

Download free faucets:

https://teztnets.xyz/hangzhounet-faucet

# Run from sources/binaries

## Create working directory

_Tezos baker requires access to the context directory, which is produced by TezEdge node._
 ```
 mkdir $HOME/data-dir-011-PtHangz2
 mkdir $HOME/data-dir-011-PtHangz2/client

 cp faucet.json $HOME/data-dir-011-PtHangz2
 ```

## Run TezEdge node

At first you need to [build TezEdge from sources](../../README.md#build-from-source-code) and then check [how to run it](../../README.md#how-to-run).

E.g. for Hangzhounet to support baking/endorsing:

_Note: This cmd runs from the main git sources directory_

_Note: The option `--disable-apply-retry=true` is indended for a Tezege node running within the network where the majority of nodes are subjects to failing with block application because of the cache issue (v11.x.x)._

```
$ LD_LIBRARY_PATH=./tezos/sys/lib_tezos/artifacts ./target/release/light-node \
 --network "hangzhounet" \
 --identity-file "$HOME/data-dir-011-PtHangz2/identity.json" \
 --identity-expected-pow 26.0 \
 --tezos-data-dir "$HOME/data-dir-011-PtHangz2/context_data" \
 --bootstrap-db-path "$HOME/data-dir-011-PtHangz2/tezedge_data" \
 --peer-thresh-low 30 --peer-thresh-high 45 \
 --protocol-runner "./target/release/protocol-runner" \
 --init-sapling-spend-params-file "./tezos/sys/lib_tezos/artifacts/sapling-spend.params" \
 --init-sapling-output-params-file "./tezos/sys/lib_tezos/artifacts/sapling-output.params" \
 --p2p-port 12534 --rpc-port 12535 \
 --tokio-threads 0 \
 --ocaml-log-enabled false \
 --tezos-context-storage=irmin \
 --log terminal \
 --log-level info \
 --log-format simple \
 --disable-apply-retry=true
Jun 10 12:29:34.855 INFO Configured network ["granadanet", "granada"] -> TEZOS_GRANADANET_2021-05-21T15:00:00Z
Jun 10 12:29:34.855 INFO Checking zcash-params for sapling... (1/5)
Jun 10 12:29:34.855 INFO Found existing zcash-params files, output_path: "/home/dev/.zcash-params/sapling-output.params", spend_path: "/home/dev/.zcash-params/sapling-spend.params", candidate_dir: "/home/dev/.zcash-params"
Jun 10 12:29:34.855 INFO Loading identity... (2/5)
Jun 10 12:29:34.855 INFO Generating new tezos identity. This will take a while, expected_pow: 26
...
```

## Build Tezos Baker/Endorser/Accuser binaries from source

Please, see [https://tezos.gitlab.io/introduction/howtoget.html#building-from-sources-via-opam](https://tezos.gitlab.io/introduction/howtoget.html#building-from-sources-via-opam)

After successfull compilation, you should see this binaries in Tezos source directory:
```
tezos-accuser-011-PtHangz2
tezos-baker-011-PtHangz2
tezos-endorser-011-PtHangz2
tezos-client
```

## Wait for TezEdge node to sync with network

```
$ tezos-client -E http:://localhost:12535 bootstrapped
Waiting for the node to be bootstrapped...
Current head: BLU4di1EGgkd (timestamp: 2021-11-05T23:19:31.000-00:00, validation: 2022-02-24T17:07:41.976-00:00)
Current head: BLKsPJN9yqs9 (timestamp: 2021-11-05T23:19:46.000-00:00, validation: 2022-02-24T17:07:42.100-00:00)
...
Current head: BLMa76HNwT1C (timestamp: 2022-02-24T19:02:54.000-00:00, validation: 2022-02-24T19:03:04.047-00:00)
Node is bootstrapped
```

## Initialize keys

_TezEdge node have to be synced already._
```
$ tezos-client \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
    activate account my_delegate with "$HOME/data-dir-011-PtHangz2/faucet.json"
...
Account my_delegate (tz1XXXXXX) activated with ꜩ76351.572618.
...
```

## Register baker/endorser delegate

_TezEdge node have to be synced already._
```
$ tezos-client \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
    register key my_delegate as delegate
...
The operation has only been included 0 blocks ago.
We recommend to wait more.
...
```

## Run baker

_Note that the delegate needs to have at least ꜩ8,000 (own or delegated funds) to get baking/endorsing rights._

_Also it takes several cycles to get baking/endorsing rights (2 + num of preserved cycles)_

_Note. For Tezos baker executable from v12.x.x `-m json` paramters should be added to make it expect JSON RPC instead of new compact encoding_

```
$ tezos-baker-011-PtHangz2 \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   run with local node "$HOME/data-dir-011-PtHangz2/context_data" my_delegate
Node is bootstrapped.
...
Baker started.
...
Feb 25 10:01:16.978 - 011-PtHangz2.delegate.baking_forge: no slot found at level 579218 (max_priority = 64)

...
```

## Run endorser

_Note that the delegate needs to have at least ꜩ8,000 (own or delegated funds) to get baking/endorsing rights._

_Also it takes several cycles to get baking/endorsing rights (2 + num of preserved cycles)_

_Note. For Tezos endorser executable from v12.x.x `-m json` paramters should be added to make it expect JSON RPC instead of new compact encoding_

```
$ tezos-endorser-011-PtHangz2 \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   run my_delegate
Node is bootstrapped.
Endorser started.
...
```

## Run accuser

```
$ tezos-accuser-011-PtHangz2 \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   run
Node is bootstrapped.
...
Accuser started.
...
```

# Use Ledger

It is possible to use hardware wallet to securely store your Tezos accounts. Currently Ledger Nano S is supported. Using Ledger Live desktop application is the easiest way to manage your Ledger Nano S.

## Initialize Ledger Nano S

You need to fully initialize your device. For details see here: http://ledger.com/start.

## Install Tezos Wallet and Tezos Baker

_To install Tezos Baker, developer mode should be enabled in Ledger Live_

For extensively detailed information about these applications, visit the developer's [GitHub page](https://github.com/obsidiansystems/ledger-app-tezos).

## Create new account

If you do not have a Tezos account in your ledger, you need to create a new one. _Note that you should not use your mainnet accounts within a testnet_.

You can create a new Tezos account using Ledger Live application.

## Import Ledger Account to Tezos Client

For this and the following steps, `tezos-client` executable will be used.

First, inspect if the ledger is recognized by the `tezos-client`

```
$ tezos-client \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   list connected ledgers
## Ledger `major-squirrel-thick-hedgehog`
Found a Tezos Wallet 2.1.0 (git-description: "091e74e9") application running
on Ledger Nano S at
[IOService:/AppleACPIPlatformExpert/PCI0@0/AppleACPIPCI/XHC1@14/XHC1@14000000/HS03@14300000/Nano
S@14300000/Nano S@0/IOUSBHostHIDDevice@14300000,0].

To use keys at BIP32 path m/44'/1729'/0'/0' (default Tezos key path), use one
of:

tezos-client import secret key ledger_username "ledger://major-squirrel-thick-hedgehog/bip25519/0h/0h"
tezos-client import secret key ledger_username "ledger://major-squirrel-thick-hedgehog/ed25519/0h/0h"
tezos-client import secret key ledger_username "ledger://major-squirrel-thick-hedgehog/secp256k1/0h/0h"
tezos-client import secret key ledger_username "ledger://major-squirrel-thick-hedgehog/P-256/0h/0h"
```

Use the second proposed command (with `ed25519` curve) to import public key from the ledger. _Note that despite the command name is `import secret key`, this is only public key that is imported._

```
$ tezos-client \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   import secret key my_delegate "ledger://major-squirrel-thick-hedgehog/ed25519/0h/0h"
```

You will need to confirm addition of the address on the ledger.

Check that the address is now known to the `tezos-client`.

```
$ tezos-client \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   list known addresses
my_delegate: tzXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX (ledger sk known)
```

## Register new account as a delegate

Now register this account in the network as a Tezos delegate:

```
$ tezos-client \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   register key my_delegate as delegate
```

To get baking/endorsing rights, this account needs owning some funds, at least ꜩ8,000 (one roll). One way of getting them is to receive them from a faucet account (see [Get free XTZ](#get-free-xtz)).

```
$ tezos-client \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
    activate account faucet with "$HOME/data-dir-011-PtHangz2/faucet.json"
...
Account faucet (tz1XXXXXX) activated with ꜩ76351.572618.
...
$ tezos-client \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   transfer 76351.572618 from faucet to my_delegate
...
```

## Run baker/endorser

Start baker daemon:

```
$ tezos-baker-011-PtHangz2 \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   run with local node "$HOME/data-dir-011-PtHangz2/context_data" my_delegate
Node is bootstrapped.
...
Baker started.
...
Feb 25 10:01:16.978 - 011-PtHangz2.delegate.baking_forge: no slot found at level 579218 (max_priority = 64)

...
```

Start endorser daemon:

```
$ tezos-endorser-011-PtHangz2 \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   run my_delegate
Node is bootstrapped.
Endorser started.
...
```

## Set up the ledger for baking

Usually you need to interact with ledger to confirm a signing operation. For baking/endorsing Tezos Baking application offers automated signing, limited to blocks and endorsements only.

To enable non-interactive singing of blocks and endorsements use the following command:

```
$ tezos-client \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   setup ledger to bake for my_delegate 
```

Make sure you confirm this operation with your ledger.

To make ledger sign blocks and endorsements non-interactively, you need to make _Tezos Baker_ application active on it.

# Remote Signing

To decouple the node processing blocks and operations and keys used to sign them _remote singer_ can be used. In this case the `tezos-baker` daemon along with a Tezedge node might be running on a cloud server, and communicating with `tezos-signer` application running on a home server with Ledger Nano S connected to it.

Tezos client binaries can communicate with `tezos-signer` process via different transports, like _http_, _https_, _tcp_ and _unix_ for unix domain socket. Also it is possible to set up authorization so it accepts signing requests only from authorized client. For more information see [Signer](http://tezos.gitlab.io/user/key-management.html#signer) in Tezos documentation.

## Set up Signing Server

Make sure that your wallet is available for Tezos client applications on the signing server. If not, you should use commands mentioned above in [Importing Ledger Account to Tezos Client](#importing-ledger-account-to-tezos-client)

Start remote signer application with schema of your choice. For example, the following command will start singning server that uses HTTP transport and listens for incoming connections on port 12536.

```
$ tezos-signer \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   launch http signer --address 0.0.0.0 -p 12536
Feb 25 18:09:09.074 - signer.http: accepting HTTP requests on port 12536
Feb 25 18:09:09.074 - signer.http: listening on address: ::ffff:0.0.0.0
```

_Note that the Ledger Nano S should be running Tezos Baking application so `tezos-signer` can use the baker account stored there_

## Run Tezedge Node and Tezos Baker with Remote Signer

Start the Tezedge node:

To make `tezos-baker` use remote signing, corresponding remote address should be added to Tezos wallet. If the home server from above is accessible via name `home`, you can use the following command:

```
$ tezos-signer \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   import secret key my_delegate http://home:12536/tz1XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
```

Now you can run `tezos-baker` using this alias:

```
$ tezos-baker-011-PtHangz2 \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   run with local node "$HOME/data-dir-011-PtHangz2/context_data" my_delegate
Node is bootstrapped.
Baker started.
...
Feb 25 10:01:16.978 - 011-PtHangz2.delegate.baking_forge: no slot found at level 579218 (max_priority = 64)

...
```

The same way you can run endorser daemon:

```
$ tezos-endorser-011-PtHangz2 \
   --endpoint "http://localhost:12535" \
   --base-dir "$HOME/data-dir-011-PtHangz2/client" \
   run my_delegate
Node is bootstrapped.
Endorser started.
...
```
