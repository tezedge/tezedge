# TezEdge [![Docs Status]][docs Link] [![Changelog][changelog-badge]][changelog] [![release-badge]][release-link] [![docker-badge]][docker-link] [![MIT licensed]][MIT link]

---
The purpose of this project is to implement a secure, trustworthy and open-source Tezos node in Rust.

In addition to implementing a new node, the project seeks to maintain and improve the Tezos ecosystem wherever possible.

## Table of Contents
* [Build status](#build-status)
* [Quick demo](#quick-demo)
    * [Prerequisites](#prerequisites)
    * [Run demo](#run-demo)
* [Documentation](#documentation)
* [How to build](#how-to-build)
    * [Supported OS distributions](#supported-os-distributions)
    * [Prerequisites installation](#prerequisites-installation)
    * [Build from source code](#build-from-source-code)
* [How to run](#how-to-run)
    * [From source with `cargo run`](#running-node-with-cargo-run)
    * [Using simplified script `run.sh`](#running-node-with-runsh-script)
    * [Run docker image](#running-node-from-docker)
    * [Graceful shutdown](#shutdown-running-node-gracefully)
* [How to use](#how-to-use)
    * [Call RPC](#example-of-how-to-call-the-rpc)
    * [Baking](#baking)
    * [Prearranged-docker-compose-files](#prearranged-docker-compose-files)
        * [Mainnet - node + explorer](#mainnet---light-node--tezedge-explorer)
        * [Mainnet - node + explorer + debugger (eBPF)](#mainnet---light-node--tezedge-explorer--tezedge-debugger)
        * [Mainnet - nodes with irmin vs memory storage + explorer](#mainnet---light-node-with-irmin-context--light-node-with-memory-context--tezedge-explorer)

[comment]: <> (        * [Sandbox - node launcher + explorer + debugger]&#40;#sandbox---sandbox-launcher--light-node--tezedge-explorer&#41;)

## Build status

---

|  CI / branch  |      master      |  develop |
|----------|:-------------:|------:|
| GitHub Actions |  [![Build Status master]][Build Link master] | [![Build Status develop]][Build Link develop] |
| Drone |  [![Drone Status master]][Drone Link]   |   [![Drone Status develop]][Drone Link] |

[Build Status master]: https://github.com/tezedge/tezedge/workflows/build/badge.svg?branch=master
[Build Status develop]: https://github.com/tezedge/tezedge/workflows/build/badge.svg?branch=develop
[Build Link master]: https://github.com/tezedge/tezedge/actions?query=workflow%3Abuild+branch%3Amaster
[Build Link develop]: https://github.com/tezedge/tezedge/actions?query=workflow%3Abuild+branch%3Adevelop

[Drone Status master]: http://ci.tezedge.com/api/badges/tezedge/tezedge/status.svg?ref=refs/heads/master
[Drone Status develop]: http://ci.tezedge.com/api/badges/tezedge/tezedge/status.svg?ref=refs/heads/develop
[Drone Link]: http://ci.tezedge.com/tezedge/tezedge/

[Docs Status]: https://img.shields.io/badge/user--docs-master-informational
[Docs Link]: http://docs.tezedge.com/

[RustDoc Status]:https://img.shields.io/badge/code--docs-master-orange

[MIT licensed]: https://img.shields.io/badge/license-MIT-blue.svg
[MIT link]: https://github.com/tezedge/tezedge/blob/master/LICENSE

[changelog]: ./CHANGELOG.md
[changelog-badge]: https://img.shields.io/badge/changelog-Changelog-%23E05735

[release-badge]: https://img.shields.io/github/v/release/tezedge/tezedge
[release-link]: https://github.com/tezedge/tezedge/releases/latest

[docker-badge]: https://img.shields.io/badge/docker-images-blue
[docker-link]: https://hub.docker.com/r/tezedge/tezedge/tags

## Quick demo

---

This demo launches two items:
- The **TezEdge node** (p2p application), which connects to the **Tezos Mainnet network**.
- The **TezEdge explorer** (web application), which connects to the TezEdge node and can be accessed from a browser to see what is going on inside the TezEdge node

### Prerequisites
If you want to run this demo, you need to first install the following:
* Git (client)
* Docker

### Run demo

1. **Download the TezEdge source code**
    ```
   # Open shell and type this code into the command line and then press Enter:
   git clone https://github.com/tezedge/tezedge
    cd tezedge
    ```
2. **Run docker (compose)**
    ```
    # Open shell and type this code into the command line and then press Enter:
    docker-compose pull
    docker-compose up
    ```
    ![alt text](https://raw.githubusercontent.com/tezedge/tezedge/master/docs/images/node_bootstrap.gif)
3. **Open your web browser by entering this address into your browser's URL bar: http://localhost:8080**
    ![alt text](https://raw.githubusercontent.com/tezedge/tezedge/master/docs/images/tezedge_explorer.gif)


_**Docker for Windows**_

The images use the hostname `localhost` to access running services.
When using docker for windows, please check:
```
docker-machine ip
```
and make sure that port forwarding is set up correctly for docker or use docker-machine resolved ip instead of `http://localhost:8080`

```
TCP ports (<host_port>:<docker_port>):
    - "80:80"
    - "4927:4927"
    - "18732:18732"
    - "19732:9732"
```

## Documentation

---
_Detailed project's documentation can be found here [Documentation][Docs Link]_


## How to build

---

### Supported OS distributions

We are linking Rust code with a pre-compiled Tezos shared library. For your convenience, we have created pre-compiled binary files
for most of the more popular Linux distributions:


|  OS  |      Versions      |
|----------|:-------------:|
| Ubuntu |  16.04, 18.04, 18.10, 19.04, 19.10, 20.04, 20.10, 21.04, 21.10 |
| Debian |  9, 10 |
| OpenSUSE |  15.1, 15.2 |
| CentOS |  8 |
| MacOS |  *experimental* - newer or equal to 10.13 should work, Intel and M1 cpus |

If you are missing support for your favorite Linux distribution, please submit a request with the [tezos-opam-builder](https://github.com/tezedge/tezos-opam-builder) project.

To build from source please follow [these instructions](tezos/interop/README.md).

### Prerequisites installation
If you want to build from source code, you need to install this before:
1. Install **Git** (client)
2. Install **Rust** command _(We recommend installing Rust through rustup.)_
    ```
    # Run the following in your terminal, then follow the onscreen instructions.
    curl https://sh.rustup.rs -sSf | sh
    ```
3. Install **Rust toolchain** _(Our releases are built with 1.58.1.)_
    ```
    rustup toolchain install 1.58.1
    rustup default 1.58.1
    ```
4. Install **required OS libs**
    - OpenSSL and Zlib
    ```
    sudo apt install openssl libssl-dev zlib1g
    ```
    - Sodiumoxide package:
    ```
    sudo apt install pkg-config libsodium-dev
    ```
    - RocksDB package:
    ```
    sudo apt install clang libclang-dev llvm llvm-dev linux-kernel-headers libev-dev
    ```
    - In macOS, using [Homebrew](https://brew.sh/):
    ```
    brew install pkg-config gmp libev libsodium hidapi libffi
    ```
   - Sandbox/wallet requirements:
    ```
    sudo apt install libhidapi-dev
    ```

### Build from source code

1. **Download TezEdge source code**
    ```
    # Open shell, type this code into the command line and then press Enter:
    git clone https://github.com/tezedge/tezedge
    cd tezedge
    ```

2. **Build**

    ```
    export SODIUM_USE_PKG_CONFIG=1
    cargo build --release
    ```

   _The node can built through the `cargo build` or `cargo build --release`, be aware, release build can take
   much longer to compile._

3. **Test**

    ```
    export SODIUM_USE_PKG_CONFIG=1
    export DYLD_LIBRARY_PATH=$(pwd)/tezos/sys/lib_tezos/artifacts # currently needed for macOS
    cargo test --release
    ```

## How to run

---

### Running node with `cargo run`

To run the node manually, you need to first build it from the source code. When put together, the node can be run, for example, like this:

```
cargo build --release
cargo run --release --bin light-node -- --config-file ./light_node/etc/tezedge/tezedge.config --protocol-runner=./target/release/protocol-runner --network=mainnet
```

All parameters can also be provided as command line arguments in the same format as in the config file, in which case
they have a higher priority than the ones in the config file. For example, we can use the default config and change the log file path:

```
cargo build --release
cargo run --release --bin light-node -- --config-file ./light_node/etc/tezedge/tezedge.config --log-file /tmp/logs/tezdge.log --protocol-runner=./target/release/protocol-runner --network=mainnet
```

_Full description of all arguments is in the light_node [README](light_node/README.md) file._

### Running node with `run.sh` script

For Linux systems, we have prepared a convenience script to run the node. It will automatically set all the necessary environmnent variables and then build and run the TezEdge node.
All arguments can be provided to the `run.sh` script in the same manner as described in the previous section.

To run the node in release mode, execute the following:

_KEEP_DATA - this flag controls, if all the target directories should be cleaned on the startup, 1 means do not clean_

```
KEEP_DATA=1 ./run.sh release --network=mainnet
```

The following command will execute the node in debug node:

```
KEEP_DATA=1 ./run.sh node --network=mainnet
```

To run the node in debug mode with an address sanitizer, execute the following:

```
KEEP_DATA=1 ./run.sh node-saddr --network=mainnet
```

You can use the docker version to build and run node from the actual source code.
- you can experiment and change source code without installing all requirements, just docker.
- you can build/run node on Windows/OSX
- this is just for development, because docker is based on full Linux (pre-build docker images are Distroless)

_If you do not need to build from souce code, just use our pre-build [docker images](#running-node-from-docker-images)_

```
./run.sh docker --network=mainnet
```

Listening for updates. Node emits statistics on the websocket server, which can be changed by `--websocket-address` argument, for example:

```
KEEP_DATA=1  ./run.sh node --network=mainnet --websocket-address 0.0.0.0:12345
```

_Full description of all arguments is in the light_node [README](light_node/README.md) file._

### Running node from `binaries`

_Note: This cmd runs from the main git sources directory_
```
LD_LIBRARY_PATH=./tezos/sys/lib_tezos/artifacts ./target/release/light-node \
    --network "mainnet" \
    --identity-file "/tmp/data-dir-mainnet/identity.json" \
    --identity-expected-pow 26.0 \
    --tezos-data-dir "/tmp/data-dir-mainnet/context_data" \
    --bootstrap-db-path "/tmp/data-dir-mainnet/tezedge_data" \
    --peer-thresh-low 30 --peer-thresh-high 45 \
    --protocol-runner "./target/release/protocol-runner" \
    --init-sapling-spend-params-file "./tezos/sys/lib_tezos/artifacts/sapling-spend.params" \
    --init-sapling-output-params-file "./tezos/sys/lib_tezos/artifacts/sapling-output.params" \
    --p2p-port 9732 --rpc-port 18732 \
    --tokio-threads 0 \
    --ocaml-log-enabled false \
    --tezos-context-storage=irmin \
    --log terminal \
    --log file \
    --log-level info \
    --log-format simple
```

### Running node from docker images

We provide automatically built images that can be downloaded from our [Docker hub][docker-link].
For instance, this can be useful when you want to run the TezEdge node in your test CI pipelines.

#### Images (distroless)
- `tezedge/tezedge:vX.Y.Z` - last versioned stable released version
- `tezedge/tezedge:latest-release` - last stable released version
- `tezedge/tezedge:latest` - actual stable development version
- `tezedge/tezedge:sandbox-vX.Y.Z` - last versioned stable released version for sandbox launcher
- `tezedge/tezedge:sandbox-latest-release` - last stable released version for sandbox launcher
- `tezedge/tezedge:sandbox-latest` - last stable released version for sandbox launcher

_More about building TezEdge docker images see [here](docker/README.md)._

#### Run image

```
docker run -i -p 9732:9732 -p 18732:18732 -p 4927:4927 -t tezedge/tezedge:v2.2.0 --network=mainnet --p2p-port 9732 --rpc-port 18732
```
_A full description of all arguments can be found in the light_node [README](light_node/README.md) file._

### Shutdown running node gracefully

Just press `Ctrl-c`, works for e.g. `cargo run` or `run.sh` script.

Or you can send a `signal` to a running process, the dedicated signal is `SIGINT`, e.g.:
```
kill -s SIGINT <PID-of-running-process-with-light-node>
```

## How to use

---

### Example of how to call the RPC

Open shell, type this code into the command line and then press Enter:

```
curl localhost:18732/chains/main/blocks/head
```

For a more detailed description of the RPCs, see the [shell](https://docs.tezedge.com/endpoints/shell) and the [protocol](https://docs.tezedge.com/endpoints/protocol) endpoints.

### Baking

- [Baking in the Hangzhou testnet](baking/011-PtHangz2/README.md)
- [Baking in the Ithaca testnet](baking/012-Psithaca/README.md)
- [Baking in the mainnet](baking/mainnet/README.md)

### Prearranged docker-compose files

#### Mainnet - light-node + tezedge explorer

**Last released version:**
```
docker-compose -f docker-compose.yml pull
docker-compose -f docker-compose.yml up
```

*(optional) Environment configuration:*

```
# (default: irmin) - choose context implementation, possible values: [irmin, tezedge, both]
TEZOS_CONTEXT_STORAGE=<possible-value>

# explorer accesses node/debugger on 'localhost' by default, you can change it like,
NODE_HOSTNAME_OR_IP=<hostname-or-ip>

e.g.:
TEZOS_CONTEXT_STORAGE=irmin NODE_HOSTNAME_OR_IP=123.123.123.123 docker-compose -f docker-compose.yml up
```

#### Mainnet - light-node + tezedge explorer + tezedge debugger

**Last released version with TezEdge Debugger with integrated eBPF**

_This requires Linux kernel at least 5.11_
```
docker-compose -f docker-compose.debug.yml pull
docker-compose -f docker-compose.debug.yml up
```

*(optional) Environment configuration:*

```
# (default: irmin) - choose context implementation, possible values: [irmin, tezedge, both]
TEZOS_CONTEXT_STORAGE=<possible-value>

# explorer accesses node/debugger on 'localhost' by default, you can change it like,
NODE_HOSTNAME_OR_IP=<hostname-or-ip>

e.g.:
TEZOS_CONTEXT_STORAGE=irmin NODE_HOSTNAME_OR_IP=123.123.123.123 docker-compose -f docker-compose.debug.yml up
```

#### Mainnet - light-node with irmin context + light-node with memory context + light-node with persistent context + tezedge-explorer

This runs two explorers:
- http://localhost:8181 - with Irmin storage
- http://localhost:8282 - with Memory storage
- http://localhost:8383 - with Persistent storage

```
# Irmin context
docker-compose -f docker-compose.storage.irmin.yml pull
docker-compose -f docker-compose.storage.irmin.yml up

# TezEdge in-memory context
docker-compose -f docker-compose.storage.memory.yml pull
docker-compose -f docker-compose.storage.memory.yml up

# TezEdge persistent context
docker-compose -f docker-compose.storage.persistent.yml pull
docker-compose -f docker-compose.storage.persistent.yml up
```

*(optional) Environment configuration:*

```
# explorer accesses node on 'localhost' by default, you can change it like,
NODE_HOSTNAME_OR_IP=<hostname-or-ip>
```

[comment]: <> (#### Sandbox - sandbox launcher + light-node + tezedge-explorer)

[comment]: <> (_See more info about sandbox [here]&#40;sandbox/README.MD&#41;_)

[comment]: <> (**Last released version:**)

[comment]: <> (```)

[comment]: <> (docker-compose -f docker-compose.sandbox.yml pull)

[comment]: <> (docker-compose -f docker-compose.sandbox.yml up)

[comment]: <> (```)

[comment]: <> (**Actual development version:**)

[comment]: <> (```)

[comment]: <> (docker-compose -f docker-compose.sandbox.latest.yml pull)

[comment]: <> (docker-compose -f docker-compose.sandbox.latest.yml up)

[comment]: <> (```)

[comment]: <> (#### Sandbox node launcher + tezedge-explorer + tezedge-debugger)

[comment]: <> (**Last released version:**)

[comment]: <> (```)

[comment]: <> (docker-compose -f docker-compose.sandbox.yml pull)

[comment]: <> (docker-compose -f docker-compose.sandbox.yml up)

[comment]: <> (```)

[comment]: <> (**Actual development version:**)

[comment]: <> (```)

[comment]: <> (docker-compose -f docker-compose.sandbox.latest.yml pull)

[comment]: <> (docker-compose -f docker-compose.sandbox.latest.yml up)

[comment]: <> (# stop and remove docker volume)

[comment]: <> (docker-compose -f docker-compose.sandbox.latest.yml down -v)

[comment]: <> (```)
