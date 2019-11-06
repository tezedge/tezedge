Tezedge
===========

[![Build Status](https://travis-ci.com/simplestaking/tezedge.svg?branch=master)](https://travis-ci.com/simplestaking/tezedge)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

The purpose of this project is to implement a secure, trustworthy, open-source Tezos node in Rust. In addition to implementing a new node, the project seeks to maintain and improve the Tezos node wherever possible. 

The project can be considered as a proof of concept. This PoC demonstrates the viability of a node build that uses Rust. 

## Components

* **light_node:** Implementation of a lightweight Tezos node written in Rust.
  If you want to run node, then enter *light_node* directory and then execute `cargo run`. For more details consult node [README](light_node/README.md) file.

* **tezos/encoding:** All incoming messages are transformed into standard Rust structures for easy manipulation using de component. This component implements serialization and deserialization of all data types used in Tezos messages.

* **tezos/crypto:** Component contains cryptographic algorithms for encryption and decryption of messages.


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
sudo apt install clang libclang-dev llvm llvm-dev linux-kernel-headers
```

Install libs required to build light-node package:
```
sudo apt install libev-dev
```

Building
--------

**4. Build node** 

On linux systems:

```
SODIUM_USE_PKG_CONFIG=1 cargo build
```

For more info on how to compile OCaml interop library please see [README](./tezos/interop/README.md).

**5. Listening for updates**

Node emits statistics on the websocket server, which can be changed with `-w` argument.
