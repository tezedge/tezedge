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

On linux systems:

```
./run.sh node
```

**5. Listening for updates**

Node emits statistics on the websocket server, which can be changed with `-w` argument.
