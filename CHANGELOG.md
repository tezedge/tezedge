# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Flag `--actions-store-backend <BACKEND1> <BACKEND2> ...`. When enabled the node stores incomming actions in one of the selected backends. Possible values are: `rocksdb`, `file`
- Flag `--kv-store-backend=STRING`. Chooses backend for data related to merkle storage. By default rocksdb database is used, possible values are :
    - `rocksdb` - persistent [RocksDB](https://rocksdb.org/) database
    - `sled` - persistent [Sled](http://sled.rs) database
    - `inmem` - volatile in memory database
    - `inmem_mark_sweep` - volatile in memory database with mark sweep garbage collector algorithm
    - `inmem_mark_move` - volatile in memory database with mark move garbage collector algorithm

### Changed

- Nothing.

### Deprecated

- Nothing.

### Removed

- light-node `--ffi-calls-gc-treshold` flag.

### Fixed

- Nothing.

### Security

- Nothing.

## [1.0.0] - 2021-02-10

### Added

- Rpc for protocol runner memory stats

### Changed

- Tokio update to 1.2.x version.
- Shell and bootstrap refactored to use kind of bootstrap pipeline to prevent stucking

### Security

- Error handling - changed expect/unwrap to errors
- Encodings - replaced recursive linked list with vector
- Encodings - introduced limits for p2p messages encoding
- Properly handle connection pool timeout
- Github Actions CI runs `cargo audit` (required)

## [0.9.2] - 2021-02-03

### Added

- Support for 008 protocol Edo + network support - p2p, rpc, fii

### Changed

- Migrated Tokio dependency from 0.2.x to 1.1.x
- RocksDB kv store splitted to three instances (db, context, context_actions)
- Reworked websocket implementation, now uses warp::ws instead of default ws
- Various changes around p2p layer and bootstrapping 

### Deprecated

- Nothing.

### Removed

- Nothing.

### Fixed

- Nothing.

### Security

- Nothing.

## [0.9.1] - 2021-01-13

### Added

- Giganode1/2 to default mainnet bootstrap peers

### Fixed

- Protocol runner restarting and IPC accept handling

## [0.9.0] - 2021-01-05

### Added

- Modification of node to be able to launch Tezos python tests in Drone CI
- Benchmarks for message encoding, ffi conversion, storage predecessor search to Drone CI
- Block applied approx. stats to log for chain_manager
- Extended statistics in merkle storage

### Changed

- Refactor shell/network channels and event/commands for actors
- Refactored chain_manager/chain_feeder + optimization to speedup bootstrap
- Optimizations of merkle storage by modifying trees in place

### Fixed

- Graceful shutdown of node and runners
- Generate invalid peer_id for identity

## [0.8.0] - 2020-11-30

### Added

- Multipass validation support for CurrentHead processing + blacklisting peers
- Support for connection to Delphinet.
- Dynamic RPC router can call Tezos's RPCs inside all protocol versions.
- Added rustfmt and clippy pipelines

### Changed

- Build is now tested on GitHub Actions instead of Travis-CI.

## [0.7.2] - 2020-11-26

### Changed

- Identity path in config for distroless docker image

## [0.7.1] - 2020-11-04

### Changed

- Logging cleanup

## [0.7.0] - 2020-10-28

### Added

- Added support for reorg + CI Drone test
- Validation for new current head after apply
- Validation for accept branch only if fitness increases
- Operation pre-validation before added to mempool

### Changed

- Skip_list was changed to merkle implementation for context

## [0.6.0] - 2020-10-20

### Added

- Added distroless docker builds
- Drone pipeline for releasing docker images (develop, master/tag)

### Fixed

- Cleanup unnecessary clones + some small optimization
- Sandbox improved error handling + cleanup


## [0.5.0] - 2020-09-30

### Added

- New OCaml FFI `ocaml-interop` integration
- Integration test for chain_manager through p2p layer

### Changed

- New library `tezos/identity` for generate/validate identity/pow in rust
- Several structs/algorithms unnecessary `clone` optimization
- Refactoring and cleanup

### Removed

- Generate identity through OCaml FFI (reimplemented in `tezos/identity`)

### Security

- Added `#![forbid(unsafe_code)]` to (almost every) modules

## [0.4.0] - 2020-09-16

### Added

- More verbose error handling in the sandbox launcher.
- New rpc `forge/operations`.
- New docker-compose file to start a setup with the sandbox launcher, tezedge-explorer front-end and tezedge-debugger.

### Fixed

- Various bugs in the sandbox launcher.


## [0.3.0] - 2020-08-31

### Added

- New configuration parameter `--disable-bootstrap-lookup` to turn off DNS lookup for peers (e.g. used for tests or sandbox).
- New configuration parameter `--db-cfg-max-threads` to better control system resources.
- New RPCs to make baking in sandbox mode possible with tezos-client.
- Support for MacOS (10.13 and newer).
- Enabling core dumps in debug mode (if not set), set max open files for process
- New sandbox module to launch the light-node via RPCs.

### Changed

- Resolved various clippy warnings/errors.
- Drone test runs offline with carthagenet-snapshoted nodes.
- New OCaml FFI - `ocaml-rs` was replaced with a new custom library based on `caml-oxide` to get GC under control and improve performance.
- P2P bootstrap process - NACK version control after metadata exchange.

## [0.2.0] - 2020-07-29

### Added

- RPCs for every protocol now support the Tezos indexer 'blockwatch/tzindex'.
- Support for connecting to Mainnet.
- Support for sandboxing, which means an empty TezEdge can be initialized with `tezos-client` for "activate protocol" and do "transfer" operation.

### Changed

- FFI upgrade based on Tezos gitlab latest-release (v7.2), now supports OCaml 4.09.1
- Support for parallel access (readonly context) to Tezos FFI OCaml runtime through r2d2 connection pooling.


## [0.1.0] - 2020-06-25

### Added

- Mempool P2P support + FFI prevalidator protocol validation.
- Support for sandboxing (used in drone tests).
- RPC for /inject/operation (draft).
- RPCs for developers for blocks and contracts.
- Possibility to run mulitple sub-process with FFI integration to OCaml.

### Changed

- Upgraded version of riker, RocksDB.
- Improved DRONE integration tests.

## [0.0.2] - 2020-06-01

### Added

- Support for connection to Carthagenet/Mainnet.
- Support for Ubuntu 20 and OpenSUSE Tumbleweed.
- RPCs for indexer blockwatch/tzindex (with drone integration test, which compares indexed data with Ocaml node against TezEdge node).
- Flags `--store-context-actions=BOOL.` If this flag is set to false, the node will persist less data to disk, which increases runtime speed.

### Changed

- P2P speed-up bootstrap - support for p2p_version 1 feature Nack_with_list, extended Nack - with potential peers to connect.

### Removed

- Storing all P2P messages (moved to tezedge-debugger), the node will persist less data to disk.

### Fixed / Security

- Remove bitvec dependency.
- Refactored FFI to Ocaml not using BigArray1's for better GC processing.

## [0.0.1] - 2020-03-31

### Added

- P2P Explorer support with dedicated RPC exposed.
- Exposed RPC for Tezos indexers.
- Ability to connect and bootstrap data from Tezos Babylonnet.
- Protocol FFI integration.

[Unreleased]: https://github.com/simplestaking/tezedge/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/simplestaking/tezedge/releases/v1.0.0
[0.9.2]: https://github.com/simplestaking/tezedge/releases/v0.9.2
[0.9.1]: https://github.com/simplestaking/tezedge/releases/v0.9.1
[0.9.0]: https://github.com/simplestaking/tezedge/releases/v0.9.0
[0.8.0]: https://github.com/simplestaking/tezedge/releases/v0.8.0
[0.7.2]: https://github.com/simplestaking/tezedge/releases/v0.7.2
[0.7.1]: https://github.com/simplestaking/tezedge/releases/v0.7.1
[0.7.0]: https://github.com/simplestaking/tezedge/releases/v0.7.0
[0.6.0]: https://github.com/simplestaking/tezedge/releases/v0.6.0
[0.5.0]: https://github.com/simplestaking/tezedge/releases/v0.5.0
[0.4.0]: https://github.com/simplestaking/tezedge/releases/v0.4.0
[0.3.0]: https://github.com/simplestaking/tezedge/releases/v0.3.0
[0.2.0]: https://github.com/simplestaking/tezedge/releases/v0.2.0
[0.1.0]: https://github.com/simplestaking/tezedge/releases/v0.1.0
[0.0.2]: https://github.com/simplestaking/tezedge/releases/v0.0.2
[0.0.1]: https://github.com/simplestaking/tezedge/releases/v0.0.1
___
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
