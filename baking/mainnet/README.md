# Running baker/endorser on Mainnet

## Table of Contents
* [Run from sources/binaries](#run-from-sourcesbinaries)
   * [Create working directory](#create-working-directory)
   * [Download snapshot data](#download-snapshot-data)
   * [Identity json file](#identity-json-file)
   * [Run TezEdge node](#run-tezedge-node)
   * [Build Tezos Baker/Endorser/Accuser binaries from source](#build-tezos-bakerendorseraccuser-binaries-from-source)
   * [Wait for TezEdge node to sync with network](#wait-for-tezedge-node-to-sync-with-network)
   * [Initialize keys for baker/endorser](#initialize-keys-for-bakerendorser)
   * [Run baker](#run-baker)
   * [Run endorser](#run-endorser)
* [Use Ledger](#use-ledger)
  * [Configure Ledger](#configure-ledger)
  * [Import Keys from Ledger](#import-keys-from-ledger)
  * [Use Ledger Key for Baking](#use-ledger-key-for-baking)
  * [Use Remote Signing](#use-remote-signing)

# Run from sources/binaries

## Create a working directory

_The Octez baker requires access to the context directory, which is produced by TezEdge or Octez node._
```
$ mkdir $HOME/data-mainnet
$ mkdir $HOME/data-mainnet/client
```

The TezEdge baker doesn't require it.

## Import snapshot data

_If you want to run node from empty storage, you can skip this step._
At first, you need to check, which (latest) snapshot to download, please visit http://snapshots.tezedge.com/ for instructions.

Example:

```
$ ./target/release/light-node import-snapshot \
   --from http://snapshots.tezedge.com:8880/mainnet/irmin/full/tezedge_mainnet_20220331-115305_BL42sTybrNx3WWdRNis76eHRXRQgVmh2oNcAb29xP473YrJSUVm_irmin.full \
   --tezos-data-dir "$HOME/data-mainnet/tezedge-data"
```

## Identity json file

a) If you already have the `identity.json` file, just copy it here
```
$ cp <your-identity.json> $HOME/data-mainnet/identity.json
```

b) If you dont have it, don't worry. The TezEdge node will generate one here:
```
$HOME/data-mainnet/identity.json
```

## Run TezEdge node

First, you need to:
- check [supported OS](../../README.md#supported-os-distributions)
- install [prerequisites](../../README.md#prerequisites-installation)
- build [TezEdge from sources](../../README.md#build-from-source-code)
- check [how to run it](../../README.md#how-to-run)

Once everything works, you can continue with launching the node.

_Note: This cmd runs from the main TezEdge git sources directory - see the relative './' paths_

```
$ LD_LIBRARY_PATH=./tezos/sys/lib_tezos/artifacts ./target/release/light-node \
    --network "mainnet" \
    --identity-file "$HOME/data-mainnet/identity.json" \
    --identity-expected-pow 26.0 \
    --tezos-data-dir "$HOME/data-mainnet/tezedge-data" \
    --peer-thresh-low 30 --peer-thresh-high 45 \
    --protocol-runner "./target/release/protocol-runner" \
    --init-sapling-spend-params-file "./tezos/sys/lib_tezos/artifacts/sapling-spend.params" \
    --init-sapling-output-params-file "./tezos/sys/lib_tezos/artifacts/sapling-output.params" \
    --p2p-port 9732 --rpc-port 8732 \
    --tokio-threads 0 \
    --ocaml-log-enabled false \
    --log terminal \
    --log-level info \
    --log-format simple

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
$ LD_LIBRARY_PATH=./tezos/sys/lib_tezos/artifacts nohup ./target/release/light-node \
    --network "mainnet" \
    --identity-file "$HOME/data-mainnet/identity.json" \
    --identity-expected-pow 26.0 \
    --tezos-data-dir "$HOME/data-mainnet/tezedge-data" \
    --peer-thresh-low 30 --peer-thresh-high 45 \
    --protocol-runner "./target/release/protocol-runner" \
    --init-sapling-spend-params-file "./tezos/sys/lib_tezos/artifacts/sapling-spend.params" \
    --init-sapling-output-params-file "./tezos/sys/lib_tezos/artifacts/sapling-output.params" \
    --p2p-port 9732 --rpc-port 8732 \
    --tokio-threads 0 \
    --ocaml-log-enabled false \
    --log terminal \
    --log-level info \
    --log-format simple &> $HOME/data-mainnet/nohup-node.out &
```

You can check the logs by typing this command:
```
$ tail -f $HOME/data-mainnet/nohup-node.out
```

## Download tezos client binaries (or build from source)

Download `tezos-client`, `tezos-baker-012-Psithaca` and `tezos-accuser-012-Psithaca` from https://gitlab.com/tezos/tezos/-/releases.

Or alternatively, build them from source. See [https://tezos.gitlab.io/introduction/howtoget.html#building-from-sources-via-opam](https://tezos.gitlab.io/introduction/howtoget.html#building-from-sources-via-opam) for instructions.

After successful compilation, you should see these binaries in Tezos source directory:
```
tezos-baker-012-Psithaca
tezos-accuser-012-Psithaca
tezos-client
```

_Note: Following commands assume that tezos sources directory is added to the PATH environment variable_

### TezEdge baker

It is recommended to use TezEdge baker instead of `tezos-baker-012-Psithaca`.

See [TezEdge Baker](../../apps/baker/README.md#running) to build and run it.

## Wait for TezEdge node to sync with the network

```
$ tezos-client bootstrapped
Disclaimer:
  The  Tezos  network  is  a  new  blockchain technology.
  Users are  solely responsible  for any risks associated
  with usage of the Tezos network.  Users should do their
  own  research to determine  if Tezos is the appropriate
  platform for their needs and should apply judgement and
  care in their network interactions.

Waiting for the node to be bootstrapped...
Current head: BLRx4bAcrxNx (timestamp: 2022-05-30T12:21:29.000-00:00, validation: 2022-05-30T16:34:53.505-00:00)
Current head: BLKjRYRUhHrz (timestamp: 2022-05-30T12:21:59.000-00:00, validation: 2022-05-30T16:34:53.940-00:00)
...
Node is bootstrapped
```

## Initialize keys for baker/endorser

See https://tezos.gitlab.io/user/key-management.html

a) If you already have existing account for baking/endorsing, you just need to import the key:
```
$ tezos-client \
  --base-dir "$HOME/data-mainnet/client" \
   import secret key <delegate_alias> <delegate_secret_key>
```

b) If you use a key provided by Ledger Nano S, import the key from it:
```
$ tezos-client \
  --base-dir "$HOME/data-mainnet/client" \
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

$ tezos-client \
  --base-dir "$HOME/data-mainnet/client" \
   import secret key <delegate_alias> "ledger://major-squirrel-thick-hedgehog/ed25519/0h/0h"
Please validate (and write down) the public key hash displayed on the Ledger,
it should be equal
to `tz1XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX`:
Tezos address added: tz1XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
```

You will need to confirm addition of the address on the ledger.

c) If you dont have any keys, you need to activate and register accounts as delegate:
```
$ tezos-client \
  --base-dir "$HOME/data-mainnet/client" \
   activate account <delegate_alias> with "<key/ledger/faucet>"
...
Account <delegate_alias> (tz1XXXXXX) activated with ꜩ76351.572618.
...
```

And then register as a delegate:

```
$ tezos-client \
  --base-dir "$HOME/data-mainnet/client" \
   register key <delegate_alias> as delegate
```

## Run Baker

### TezEdge baker

Assuming that the secret key (or locator for remote signing) is in `$HOME/.tezos-client` and the node is running locally and uses 18732 port for RPC, the command is:

```
$ tezedge-baker --base-dir "$HOME/.tezos-client" --endpoint "http://localhost:18732" --baker <delegate_alias>
```

### Octez baker

_Note. For Tezos baker executable from v12.x.x `--media-type json` (or `-m json`) parameters should be added to make it expect JSON RPC instead of new compact encoding_

```
$ tezos-baker-012-Psithaca \
  --media-type json \
  --base-dir "$HOME/data-mainnet/client" \
  run with local node "$HOME/data-mainnet" <delegate_alias>

Node is bootstrapped.
...
Baker started.
...
```

## Run accuser

```
$ tezos-accuser-012-Psithaca \
   --media-type json \
   --base-dir "$HOME/data-mainnet/client" \
   run
Node is bootstrapped.
...
Accuser started.
...
```


# Use Ledger

It is possible to use hardware wallet to securely store your Tezos accounts. Currently Ledger Nano S is supported. Using Ledger Live desktop application is the easiest way to manage your Ledger Nano S.

Ledger Nano S can be managed by Ledger Live applicaton. Most of Tezos related operations should be performed with `tezos-client` binary.

For more information on using Ledger Nano S, please visit the following sites:
- https://ledger.com/start
- http://tezos.gitlab.io/user/key-management.html#ledger-support
- https://github.com/obsidiansystems/ledger-app-tezos

## Configure Ledger

You need to fully initialize your device. For details see here: http://ledger.com/start.

After your device is initialized, you need to install Tezos Wallet application on it in order to create a new Tezos keys. After your keys are created, you also need to get some funds (at least ꜩ8,000) so you can start baking blocks.

## Import Keys from Ledger

First, inspect if the ledger is recognized by the `tezos-client`

```
$ tezos-client \
   --base-dir "$HOME/data-mainnet/client" \
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
   --base-dir "$HOME/data-mainnet/client" \
   import secret key my_delegate "ledger://major-squirrel-thick-hedgehog/ed25519/0h/0h"
```

You will need to confirm addition of the address on the ledger.

Check that the address is now known to the `tezos-client`.

```
$ tezos-client \
   --base-dir "$HOME/data-mainnet/client" \
   list known addresses
my_delegate: tzXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX (ledger sk known)
```

## Use Ledger Key for Baking

Usually you need to interact with ledger to confirm a signing operation. For baking/endorsing Tezos Baking application offers automated signing, limited to blocks and endorsements only.

To enable non-interactive singing of blocks and endorsements use the following command:

```
$ tezos-client \
   --base-dir "$HOME/data-mainnet/client" \
   setup ledger to bake for my_delegate
```

Make sure you confirm this operation with your ledger.

To make ledger sign blocks and endorsements non-interactively, you need to make _Tezos Baking_ application active on it.

## Use Remote Signing

To decouple the node processing blocks and operations and keys used to sign them _remote singer_ can be used. In this case the `tezos-baker` daemon along with a Tezedge node might be running on a cloud server, and communicating with `tezos-signer` application running on a home server with Ledger Nano S connected to it.

Tezos client binaries can communicate with `tezos-signer` process via different transports, like _http_, _https_, _tcp_ and _unix_ for unix domain socket. Also it is possible to set up authorization so it accepts signing requests only from authorized client. For more information see [Signer](http://tezos.gitlab.io/user/key-management.html#signer) in Tezos documentation. Another layer of security can be added by using _https_ schema or wrapping unencrypted traffic into e.g. an ssh tunnel._

Make sure that your wallet is available for Tezos client applications on the signing server. If not, you should use commands mentioned above in [Import Keys from Ledger](#import-keys-from-ledger).

Start remote signer application with schema of your choice. For example, the following command will start singning server that uses HTTP transport and listens for incoming connections on port 17732.
```
$ tezos-signer \
   --base-dir "$HOME/data-mainnet/client" \
   launch http signer --address 0.0.0.0 -p 17732
Feb 25 18:09:09.074 - signer.http: accepting HTTP requests on port 17732
Feb 25 18:09:09.074 - signer.http: listening on address: ::ffff:0.0.0.0
```

_Note that the Ledger Nano S should be running Tezos Baker application so `tezos-signer` can use the baker account stored there_

To make `tezos-baker` use remote signing, corresponding remote address should be added to Tezos wallet. If the home server from above is accessible via name `home`, you can use the following command:

```
$ tezos-signer \
   --base-dir "$HOME/data-mainnet/client" \
   import secret key my_delegate http://home:17732/tz1XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
```

Now you can run `tezos-baker` using this alias:

```
$ tezos-baker-012-Psithaca \
   --media-type json \
   --base-dir "$HOME/data-mainnet/client" \
   run with local node "$HOME/data-mainnet/context_data" my_delegate
Node is bootstrapped.
Baker started.
...
```
