# TezEdge

[![Docs Status]][docs Link]
[![Changelog][changelog-badge]][changelog]
[![release-badge]][release-link]
[![docker-badge]][docker-link]
[![MIT licensed]][MIT link]

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

The purpose of this project is to implement a secure, trustworthy, open-source Tezos node in Rust.
In addition to implementing a new node, the project seeks to maintain and improve the Tezos node wherever possible.

[Documentation][Docs Link]

Quick start
------------

**Pre-requisites**

* GitHub repository

* Docker

**1. Open shell and type this code into the command line and then press Enter**

```
git clone https://github.com/simplestaking/tezedge
cd tezedge
```

**2. Download and install Docker and Docker Compose**

Open shell and type this code into the command line and then press Enter:

```
docker-compose pull
docker-compose up
```

![alt text](https://raw.githubusercontent.com/simplestaking/tezedge/master/docs/images/node_bootstrap.gif)

**Docker for Windows**

Images use hostname `localhost` to access running services.
When using docker for windows, check, please:
```
docker-machine ip
```
and make sure that port forwarding is set up correctly for docker.

**3. Open the TezEdge Explorer in your browser**

You can view the status of the node in your browser by entering this address into your browser's URL bar:

http://localhost:8080

![alt text](https://raw.githubusercontent.com/simplestaking/tezedge/master/docs/images/tezedge_explorer.gif)

Building from Source
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
rustup toolchain install nightly-2020-10-24
rustup default nightly-2020-10-24
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

In OSX, using [Homebrew](https://brew.sh/):
```
brew install pkg-config gmp libev libsodium hidapi
```

Install libs required to build sandbox:
```
sudo apt install libhidapi-dev
```

**4. Supported OS distributions**

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

Running node manually
----------------

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
they have higher priority than the ones in config file. For example we can use the default config and change the log file path:
```
LD_LIBRARY_PATH=./tezos/interop/lib_tezos/artifacts cargo run --bin light-node -- --config-file ./light_node/etc/tezedge/tezedge.config --log-file /tmp/logs/tezdge.log
```

Full description of all arguments is in the light_node [README](light_node/README.md) file.



Running node using run.sh script
----------------


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

Running node using prearranged docker-compose files
----------------

### light-node + tezedge-explorer
* Run last released version:
```
docker-compose -f docker-compose.yml pull
docker-compose -f docker-compose.yml up
```
* Run actual development version:
```
docker-compose -f docker-compose.latest.yml pull
docker-compose -f docker-compose.latest.yml up
```

### sandbox launcher + tezedge-explorer + tezedge-debugger
* Run last released version:
```
docker-compose -f docker-compose.sandbox.yml pull
docker-compose -f docker-compose.sandbox.yml up
```
* Run actual development version:
```
docker-compose -f docker-compose.sandbox.latest.yml pull
docker-compose -f docker-compose.sandbox.latest.yml up

# stop and remove docker volume
docker-compose -f docker-compose.sandbox.latest.yml down -v
```

Shutdown running node gracefully
----------------
Just press `Ctrl-c`, works for e.g. `cargo run` or `run.sh` script.

Or you can send `signal` to running process, dedicated signal is `SIGINT`, e.g.:
```
kill -s SIGINT <PID-of-running-process-with-light-node>
```

Example of how to call the RPC
----------------

Open shell and type this code into the command line and then press Enter:

```curl localhost:18732/chains/main/blocks/head```

For a more detailed description of RPCs, see the [shell](https://docs.tezedge.com/endpoints/shell) and the [protocol](https://docs.tezedge.com/endpoints/protocol) endpoints.