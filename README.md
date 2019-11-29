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
rustup toolchain install nightly-2019-11-29
rustup default nightly-2019-11-29
```

**3. Install required libs**

Install libs required to build sodiumoxide package:
```
sudo apt install pkg-config libsodium-dev
```

Install libs required to build RocksDB package:
```
sudo apt install clang libclang-dev llvm llvm-dev linux-kernel-headers libev-dev
```

Running the node
------------

**4. Running Tezedge node manually** 

The node can built through the `cargo build` or `cargo build --release`, be aware, release build can take 
much longer to compile. environment variable `SODIUM_USE_PKG_CONFIG=1` mus be set. Put together, node can be build, for example, like this:
```
SODIUM_USE_PKG_CONFIG=1 cargo build
```

To run node manually, path to the Tezos lib must be provided as environment variable `LD_LIBRARY_PATH`. It is required
by `protocol-runner`. Put together, node can be run, for example, like this:
```
LD_LIBRARY_PATH=./tezos/interop/lib_tezos/artifacts cargo run --bin light-node -- --config-file ./light_node/etc/tezedge/tezedge.config
```

All parameters can be provided also as command line arguments in the same format as in config file, in which case 
they have higher priority than the ones in conifg file. For example we can use the default config and change the log file path:
```
LD_LIBRARY_PATH=./tezos/interop/lib_tezos/artifacts cargo run --bin light-node -- --config-file ./light_node/etc/tezedge/tezedge.config --log-file /tmp/logs/tezdge.log
```

Full description of all arguments is in the light_node [README](light_node/README.md) file.



**5. Running Tezedge node using run.sh script** 

On linux systems, we prepared convenience script to run the node. It will automatically set all necessary environmnent variables, build and run tezedge node. 
All arguments can be provided to the `run.sh` script in the same manner as described in the previous section - Running Tezedge node manually.

The following command will execute node in debug node:

```
./run.sh node
```

To run node in release mode execute the following:

```
./run.sh release
```

If you are running OSX you can use docker version:

```
./run.sh docker
```

Listening for updates. Node emits statistics on the websocket server, which can be changed by `--websocket-address` argument, for example:

```
./run.sh node --websocket-address 0.0.0.0:12345
```