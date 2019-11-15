Tezedge
===========

[![Build Status](https://travis-ci.com/simplestaking/tezedge.svg?branch=master)](https://travis-ci.com/simplestaking/tezedge)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

The purpose of this project is to implement a secure, trustworthy, open-source Tezos node in Rust.
In addition to implementing a new node, the project seeks to maintain and improve the Tezos node wherever possible. 

Requirements
------------

**1. Install rustup command** 

We recommend installing Rust through rustup.

Run the following in your terminal, then follow the onscreen instructions.

```
curl https://sh.rustup.rs -sSf | sh
```

**2. Install rust toolchain** 

Rust nightly is required to build this project.
```
rustup toolchain install nightly-2019-10-14
rustup default nightly-2019-10-14
```
Application has been tested to compile with `rustc 1.40.0-nightly (e413dc36a 2019-10-14)`.

**3. Install required libs**

Install libs required to build sodiumoxide package:
```
sudo apt install libsodium-dev
```

Install libs required to build RocksDB package:
```
sudo apt install clang libclang-dev llvm llvm-dev linux-kernel-headers libev-dev
```

Launching node
--------

**4. Start Tezedge node** 

On linux systems, we prepared convenience script to run the node:

```
./run.sh node
```

if you are running OSX you can use docker version:

```
./run docker
```

**5. Listening for updates**

Node emits statistics on the websocket server, which can be changed with `-w` argument, for example:

```
./run.sh node -w 0.0.0.0:12345
```

Running node
=========
The node can built and run manually through the `cargo build` or `cargo build --release`, be aware, release build can take 
much longer to compile. Running node with`cargo run` and correct arguments, described in following sections.

All arguments can be provided to the `run.sh` script.


Required arguments
-----

### Identity 
Path to your Tezos `identity.json` file.
```
-i, --identity <PATH>
```

### Network
Specify the Tezos environment for this node. Accepted values are: 
`alphanet, babylonnet, babylon, mainnet or zeronet`, where `babylon` and `babylonnet` refer to same environment.
```
-n, --network [alphanet, babylonnet, babylon, mainnet, zeronet]
```

### Protocol runner
Path to the protocol runner binary, which is compiled with `tezedge`. 
For example: `./target/debug/protocol-runner`.
```
-P, --protocol-runner <PATH>
```

Full description of all arguments is in the light_node [README](light_node/README.md) file.

Manually running the node
-----
Last step is to provide path to the Tezos lib, which is set as environment variable `LD_LIBRARY_PATH`, as is required
by `protocol-runner`. Put together, node can be run, for example, like this:
```
# LD_LIBRARY_PATH=./tezos/interop/lib_tezos/artifacts cargo run -- -i ligth_node/config/identity.json -n babylon -P ./target/debug/protocol-runner
```
