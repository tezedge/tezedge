# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Nothing.

### Changed

- Nothing.

### Deprecated

- Nothing.

### Removed

- Nothing.

### Fixed

- Nothing.

### Security

- Nothing.

## [1.7.1] - 2021-08-31

### Fixed

- Fix a corner case in the communication with the protocol runner that could cause some off-by-one responses.

## [1.7.0] - 2021-08-19

### Added

- Bootstrap time test to Drone CI
- Implemented inodes representation in the TezEdge context storage

### Changed

- Upgrade rust nightly to 2021-08-04
- Drone CI test pipelines optimization and improvements
- Drone CI test pipelines optimization and improvements

### Fixed

- Be more robust on the handling of IPC errors
- Calculate block header hash on decoding optimization
- TezEdge context fixes, optimizations and improvements
- Fixed chrono dependency panics
- Reworked <block_id> parsing for rpc and better error handling (future blocks, ...)

### Removed

- Remove no-longer used COPY context function

## [1.6.10] - 2021-07-30

### Changed

- New irmin context version

## [1.6.9] - 2021-07-22

### Changed

- RPC uses protocol runners without context for decoding block/operations metadata

## [1.6.8] - 2021-07-20

### Fixed

- Add dns/resolv libs to distroless docker image

## [1.6.7] - 2021-07-20

### Added

- Quota throttling for p2p messages

## [1.6.6] - 2021-07-16

### Added

- Add massif test for bootstrapping

### Changed

- Upgrade tokio dependency to 1.8

### Fixed

- Cleaning and better error handling

## [1.6.5] - 2021-07-13

### Added

- Add support for custom networks specified by a config file
- Log configuration on startup

### Fixed

- Storage db compatibility for 19 vs 20 for snapshots

## [1.6.4] - 2021-07-12

### Changed

- Slog default rolling parameters to save more logs

## [1.6.2] - 2021-07-07

### Fixed

- Block storage iterator was returning blocks in reverse order.

## [1.6.1] - 2021-07-07

### Added

- A reworked in-memory backend for the TezEdge context that conforms to the Tezos context API and is now directly accessed from the Tezos protocol code.
- Flag `--tezos-context-storage` to choose the context backend. Default is `irmin`, supported values are:
  - `tezedge` - Use the TezEdge context backend.
  - `irmin` - Use the Irmin context backend.
  - `both` - Use both backends at the same time
- `inmem-gc` option to the `--context-kv-store` flag.
- Flag `--context-stats-db-path=<PATH>` that enables the context storage stats. When this option is enabled, the node will measure the time it takes to complete each context query. When available, these will be rendered in the TezEdge explorer UI.
- A new `replay` subcommand to the `light-node` program. This subcommand will take as input a range of blocks, a blocks store and re-apply all those blocks to the context store and validate the results.
- A new CI runner running on linux with real-time patch kernel to increase determinism of performance tests
- Add conseil and tzkt tests for florencenet
- Add caching to functions used by RPC handlers

### Changed

- Implemented new storage based on sled as replacement for rocksdb
- Implemented new commit log as storage for plain bytes/json data for block_headers
- New dockerhub: simplestakingcom -> tezedge

### Removed

- Flag `--one-context` was removed, now all context backends are accessed directly by the protocol runner.
- RocksDB and Sled backends are not supported anymore by the TezEdge context.
- The actions store enabled by `--actions-store-backend` is currently disabled and will not record anything.

## [1.5.1] - 2021-06-08

### Added

- Preserve frame pointer configuration (used by eBPF memprof docker image)

### Changed

- Updated docker-composes + README + sandbox update for 009/010

## [1.5.0] - 2021-06-06

### Added

- New protocol 009_Florence/010_Granada (baker/endorser) integration support + (p2p, protocols, rpc, tests, CI pipelines)

### Changed

- Encodings - implemented NOM decoding
- (FFI) Compatibility with Tezos v9-release
- Store apply block results (header/operations) metadata as plain bytes and added rpc decoding to speedup block application
- TezEdge node now works just with one Irmin context (temporary solution, custom context is coming soon...)

## [1.4.0] - 2021-05-20

### Changed
- Optimize the Staging Area Tree part of Context Storage
- Shell memory optimizations
- Changed bootstrap current_branch/head algo
- New Drone CI with parallel runs

### Security

- Added Proof-of-work checking to hand-shake

## [1.3.1] - 2021-04-14

### Added

- New module `deploy_monitoring` for provisioning of TezEdge node, which runs as docker image with TezEdge Debugger and TezEdge Explorer
- Flag `--one-context` to turn-off TezEdge second context and use just one in the FFI
- Peer manager stats to log
- More tests to networking layer

### Fixed
- P2p manager limit incoming connections by ticketing
- Dead-lettered peer actors cleanup
- Memory RAM optimization

## [1.2.0] - 2021-03-26

### Added

- Automatically generated documentation on P2P messages encoding.
- Context actions record/replay feature
- Flag `--actions-store-backend <BACKEND1> <BACKEND2> ...`. When enabled the node stores incomming actions in one of the selected backends. Possible values are: `rocksdb`, `file`
- Flag `--context-kv-store=STRING`. Chooses backend for data related to merkle storage. By default rocksdb database is used, possible values are :
  - `rocksdb` - persistent [RocksDB](https://rocksdb.org/) database
  - `sled` - persistent [Sled](http://sled.rs) database
  - `inmem` - volatile in memory database(unordered)
  - `btree` - volatile in memory database(ordered)
- Added new RPCs for get operations details

### Changed

- Storage module refactor
- Upgrade code to ocaml-interop v0.7.2

### Security

- Safer handling of String encoding

## [1.1.4] - 2021-03-12

### Added

- Extended tests for calculation of context_hash
- Possibility to use multiple logger (terminal and/or file)

### Changed

- Shell refactor to batch block application to context

## [1.1.3] - 2021-03-09

### Fixed

- Correct parsing of bootstrap addresses with port

### Deprecated

- edo/edonet - automatically is switched to edo2/edo2net

## [1.1.2] - 2021-03-05

### Added

- New 008 edo2 support + possibility to connect to edo2net
- New algorithm for calculation of context_hash according to Tezos

## [1.1.1] - 2021-03-05

### Fixed
- README.md and predefined docker composes

## [1.1.0] - 2021-03-02

### Added

- Sapling zcash-params init configuration handling for edo protocol on startup
- Backtracking support for Merkle storage

### Changed

- Argument `--network=` is required + possibility to run dockers with different networks
- RPC integration tests optimization - run in parallel and add elapsed time to the result in Drone CI
- Minor changes for dev RPCs for TezEdge-Explorer

### Removed

- Argument `--ffi-calls-gc-treshold`.
- Default value for argument `--network=`

### Fixed

- Used hash instead of string for peer_id in SwapMessage

### Security

- Added limits for p2p messages according to the Tezos updates

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
- RocksDB kv store splitted into three instances (db, context, context_actions)
- Reworked websocket implementation, now uses warp::ws instead of default ws
- Various changes around the P2P layer and bootstrapping

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

[Unreleased]: https://github.com/tezedge/tezedge/compare/v1.7.1...HEAD
[1.7.1]: https://github.com/tezedge/tezedge/releases/v1.7.1
[1.7.0]: https://github.com/tezedge/tezedge/releases/v1.7.0
[1.6.10]: https://github.com/tezedge/tezedge/releases/v1.6.10
[1.6.9]: https://github.com/tezedge/tezedge/releases/v1.6.9
[1.6.8]: https://github.com/tezedge/tezedge/releases/v1.6.8
[1.6.7]: https://github.com/tezedge/tezedge/releases/v1.6.7
[1.6.6]: https://github.com/tezedge/tezedge/releases/v1.6.6
[1.6.5]: https://github.com/tezedge/tezedge/releases/v1.6.5
[1.6.4]: https://github.com/tezedge/tezedge/releases/v1.6.4
[1.6.2]: https://github.com/tezedge/tezedge/releases/v1.6.2
[1.6.1]: https://github.com/tezedge/tezedge/releases/v1.6.1
[1.6.0]: https://github.com/tezedge/tezedge/releases/v1.6.0
[1.5.1]: https://github.com/tezedge/tezedge/releases/v1.5.1
[1.5.0]: https://github.com/tezedge/tezedge/releases/v1.5.0
[1.4.0]: https://github.com/tezedge/tezedge/releases/v1.4.0
[1.3.1]: https://github.com/tezedge/tezedge/releases/v1.3.1
[1.2.0]: https://github.com/tezedge/tezedge/releases/v1.2.0
[1.1.4]: https://github.com/tezedge/tezedge/releases/v1.1.4
[1.1.3]: https://github.com/tezedge/tezedge/releases/v1.1.3
[1.1.2]: https://github.com/tezedge/tezedge/releases/v1.1.2
[1.1.0]: https://github.com/tezedge/tezedge/releases/v1.1.0
[1.0.0]: https://github.com/tezedge/tezedge/releases/v1.0.0
[0.9.2]: https://github.com/tezedge/tezedge/releases/v0.9.2
[0.9.1]: https://github.com/tezedge/tezedge/releases/v0.9.1
[0.9.0]: https://github.com/tezedge/tezedge/releases/v0.9.0
[0.8.0]: https://github.com/tezedge/tezedge/releases/v0.8.0
[0.7.2]: https://github.com/tezedge/tezedge/releases/v0.7.2
[0.7.1]: https://github.com/tezedge/tezedge/releases/v0.7.1
[0.7.0]: https://github.com/tezedge/tezedge/releases/v0.7.0
[0.6.0]: https://github.com/tezedge/tezedge/releases/v0.6.0
[0.5.0]: https://github.com/tezedge/tezedge/releases/v0.5.0
[0.4.0]: https://github.com/tezedge/tezedge/releases/v0.4.0
[0.3.0]: https://github.com/tezedge/tezedge/releases/v0.3.0
[0.2.0]: https://github.com/tezedge/tezedge/releases/v0.2.0
[0.1.0]: https://github.com/tezedge/tezedge/releases/v0.1.0
[0.0.2]: https://github.com/tezedge/tezedge/releases/v0.0.2
[0.0.1]: https://github.com/tezedge/tezedge/releases/v0.0.1
___
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
