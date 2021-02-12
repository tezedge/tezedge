# TezEdge [![Docs Status]][docs Link] [![Changelog][changelog-badge]][changelog] [![release-badge]][release-link] [![docker-badge]][docker-link] [![MIT licensed]][MIT link]

---
The purpose of this project is to implement a secure, trustworthy, open-source Tezos node in Rust.

In addition to implementing a new node, the project seeks to maintain and improve the Tezos node wherever possible.

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
    * [Prearranged-docker-compose-files](#prearranged-docker-compose-files)
        * [Mainnet - node + explorer](#mainnet---light-node--tezedge-explorer)
        * [Sandbox - node launcher + explorer + debugger](#sandbox-node-launcher--tezedge-explorer--tezedge-debugger)

## Build status

---

|  CI / branch  |      master      |  develop |
|----------|:-------------:|------:|
| GitHub Actions |  [![Build Status master]][Build Link master] | [![Build Status develop]][Build Link develop] |
| Drone |  [![Drone Status master]][Drone Link]   |   [![Drone Status develop]][Drone Link] |

[Build Status master]: https://github.com/simplestaking/tezedge/workflows/build/badge.svg?branch=master
[Build Status develop]: https://github.com/simplestaking/tezedge/workflows/build/badge.svg?branch=develop
[Build Link master]: https://github.com/simplestaking/tezedge/actions?query=workflow%3Abuild+branch%3Amaster
[Build Link develop]: https://github.com/simplestaking/tezedge/actions?query=workflow%3Abuild+branch%3Adevelop

[Drone Status master]: http://ci.tezedge.com/api/badges/simplestaking/tezedge/status.svg?ref=refs/heads/master
[Drone Status develop]: http://ci.tezedge.com/api/badges/simplestaking/tezedge/status.svg?ref=refs/heads/develop
[Drone Link]: http://ci.tezedge.com/simplestaking/tezedge/

[Docs Status]: https://img.shields.io/badge/user--docs-master-informational
[Docs Link]: http://docs.tezedge.com/

[RustDoc Status]:https://img.shields.io/badge/code--docs-master-orange

[MIT licensed]: https://img.shields.io/badge/license-MIT-blue.svg
[MIT link]: https://github.com/simplestaking/tezedge/blob/master/LICENSE

[changelog]: ./CHANGELOG.md
[changelog-badge]: https://img.shields.io/badge/changelog-Changelog-%23E05735

[release-badge]: https://img.shields.io/github/v/release/simplestaking/tezedge
[release-link]: https://github.com/simplestaking/tezedge/releases/latest

[docker-badge]: https://img.shields.io/badge/docker-images-blue
[docker-link]: https://hub.docker.com/r/simplestakingcom/tezedge/tags

## Quick demo

---

This demo runs two thing things:
- **TezEdge node** (p2p application), which connects to the **Tezos Mainnet network**.
- **TezEdge explorer** (web application), which connects to the TezEdge node, can be accessed from browser to see what is going on inside TezEdge node

### Prerequisites
If you want to run this demo, you need to install this before:
* Git (client)
* Docker

### Run demo

1. **Download TezEdge source code**
    ```
   # Open shell and type this code into the command line and then press Enter:
   git clone https://github.com/simplestaking/tezedge
    cd tezedge
    ```
2. **Run docker (compose)**
    ```
    # Open shell and type this code into the command line and then press Enter:
    docker-compose pull
    docker-compose up
    ```
    ![alt text](https://raw.githubusercontent.com/simplestaking/tezedge/master/docs/images/node_bootstrap.gif)
3. **Open your web browser by entering this address into your browser's URL bar: http://localhost:8080**
    ![alt text](https://raw.githubusercontent.com/simplestaking/tezedge/master/docs/images/tezedge_explorer.gif)


_**Docker for Windows**_

The images use hostname `localhost` to access running services.
When using docker for windows, check, please:
```
docker-machine ip
```
and make sure that port forwarding is set up correctly for docker or use docker-machine resolved ip instead of `http://localhost:8080`

## Documentation

---
_Detailed project's documentation can be found here [Documentation][Docs Link]_


## How to build

---

### Supported OS distributions

We are linking rust code with pre-compiled Tezos shared library. For your convenience we have created pre-compiled binary files
for most of the popular linux distributions:


|  OS  |      Versions      |
|----------|:-------------:|
| Ubuntu |  16.04, 18.04, 18.10, 19.04, 19.10, 20.04, 20.10 |
| Debian |  9, 10 |
| OpenSUSE |  15.1, 15.2 |
| CentOS |  8 |
| MacOS |  *experimental* - newer or equal to 10.13 should work |

If you are missing support for your favorite linux distribution on a poll request at [tezos-opam-builder](https://github.com/simplestaking/tezos-opam-builder) project.

### Prerequisites installation
If you want to build from source code, you need to install this before:
1. Install **Git** (client)
2. Install **Rust** command _(We recommend installing Rust through rustup.)_
    ```
    # Run the following in your terminal, then follow the onscreen instructions.
    curl https://sh.rustup.rs -sSf | sh
    ```
3. Install **Rust toolchain** _(Rust nightly is required to build this project.)_
    ```
    rustup toolchain install nightly-2020-12-31
    rustup default nightly-2020-12-31
    ```
4. Install **required OS libs**
    - Sodiumoxide package:
    ```
    sudo apt install pkg-config libsodium-dev
    ```
    - RocksDB package:
    ```
    sudo apt install clang libclang-dev llvm llvm-dev linux-kernel-headers libev-dev
    ```
    - In OSX, using [Homebrew](https://brew.sh/):
    ```
    brew install pkg-config gmp libev libsodium hidapi
    ```
   - Sandbox/wallet requirements:
    ```
    sudo apt install libhidapi-dev
    ```

### Build from source code

1. **Download TezEdge source code**
    ```
    # Open shell and type this code into the command line and then press Enter:
    git clone https://github.com/simplestaking/tezedge
    cd tezedge
    ```

2. **Build**
    ```
    SODIUM_USE_PKG_CONFIG=1 cargo build --release
    ```
    or
    ```
    export SODIUM_USE_PKG_CONFIG=1
    cargo build --release
    ```
   _The node can built through the `cargo build` or `cargo build --release`, be aware, release build can take
   much longer to compile._

3. **Test**
    ```
    SODIUM_USE_PKG_CONFIG=1 cargo test --release
    ```
   or
    ```
    export SODIUM_USE_PKG_CONFIG=1
    cargo test --release
    ```

## How to run

---

### Running node with `cargo run`

To run node manually, you need to build it from source code at first. Path to the Tezos lib must be provided as environment variable `LD_LIBRARY_PATH`. It is required
by `protocol-runner`. Put together, node can be run, for example, like this:
```
LD_LIBRARY_PATH=./tezos/interop/lib_tezos/artifacts cargo run --bin light-node -- --config-file ./light_node/etc/tezedge/tezedge.config --network=mainnet
```

All parameters can be provided also as command line arguments in the same format as in config file, in which case
they have higher priority than the ones in config file. For example we can use the default config and change the log file path:
```
LD_LIBRARY_PATH=./tezos/interop/lib_tezos/artifacts cargo run --bin light-node -- --config-file ./light_node/etc/tezedge/tezedge.config --log-file /tmp/logs/tezdge.log --network=mainnet
```
_Full description of all arguments is in the light_node [README](light_node/README.md) file._

### Running node with `run.sh` script

On linux systems, we prepared convenience script to run the node. It will automatically set all necessary environmnent variables, build and run tezedge node.
All arguments can be provided to the `run.sh` script in the same manner as described in the previous section.

To run node in release mode execute the following:

```
./run.sh release --network=mainnet
```

The following command will execute node in debug node:

```
./run.sh node --network=mainnet
```

To run node in debug mode with address sanitizer execute the following:

```
./run.sh node-saddr --network=mainnet
```

If you are running OSX you can use docker version:

```
./run.sh docker --network=mainnet
```

Listening for updates. Node emits statistics on the websocket server, which can be changed by `--websocket-address` argument, for example:

```
./run.sh node --network=mainnet --websocket-address 0.0.0.0:12345
```

_Full description of all arguments is in the light_node [README](light_node/README.md) file._

### Running node from docker images

We provide automatically build images, that can be downloaded from our [Docker hub][docker-link].
This can be useful, if you want to run e.g. TezEdge node in your test CI pipelines.

#### Images (distroless)
- `simplestakingcom/tezedge:latest-release` - last stable released version
- `simplestakingcom/tezedge:latest` - actual stable development version
- `simplestakingcom/tezedge:sandbox-latest-release` - last stable released version for sandbox launcher
- `simplestakingcom/tezedge:sandbox-latest` - last stable released version for sandbox launcher

_More about building TezEdge docker images see [here](docker/README.md)._

#### Run image

```
docker run -i -t simplestakingcom/tezedge:latest --network=mainnet --p2p-port=9732
```
_Full description of all arguments is in the light_node [README](light_node/README.md) file._

### Shutdown running node gracefully

Just press `Ctrl-c`, works for e.g. `cargo run` or `run.sh` script.

Or you can send `signal` to running process, dedicated signal is `SIGINT`, e.g.:
```
kill -s SIGINT <PID-of-running-process-with-light-node>
```

## How to use

---

### Example of how to call the RPC

Open shell and type this code into the command line and then press Enter:

```
curl localhost:18732/chains/main/blocks/head
```

For a more detailed description of RPCs, see the [shell](https://docs.tezedge.com/endpoints/shell) and the [protocol](https://docs.tezedge.com/endpoints/protocol) endpoints.

### Prearranged docker-compose files

#### Mainnet - light-node + tezedge-explorer
**Last released version:**
```
docker-compose -f docker-compose.yml pull
docker-compose -f docker-compose.yml up
```
**Actual development version:**
```
docker-compose -f docker-compose.latest.yml pull
docker-compose -f docker-compose.latest.yml up
```

#### Sandbox node launcher + tezedge-explorer + tezedge-debugger
**Last released version:**
```
docker-compose -f docker-compose.sandbox.yml pull
docker-compose -f docker-compose.sandbox.yml up
```
**Actual development version:**
```
docker-compose -f docker-compose.sandbox.latest.yml pull
docker-compose -f docker-compose.sandbox.latest.yml up

# stop and remove docker volume
docker-compose -f docker-compose.sandbox.latest.yml down -v
```