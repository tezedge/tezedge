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

### Performance

- Nothing.

## [1.16.1] - 2022-03-04

### Added

- Nothing.

### Changed

- Rust 2021 edition is used.
- The option `--disable-apply-retry` now is `false` by default.

### Deprecated

- Nothing.

### Removed

- Nothing.

### Fixed

- Fixed `/chains/:chain-id:/blocks` RPC that prevented seed nonces to be revealed.
- Fixed potential node stucking when using `--disable-apply-retry=true`.
- Fixed issue that caused the snapshot command to sometimes timeout on slower computers.

### Security

- RUSTSEC-2020-0071 is fixed by using newer `time` crate version.
- RUSTSEC-2020-0159 is fixed by using `time` crate instead of `chrono`.

### Performance

- Nothing.

## [1.16.0] - 2022-02-28

### Added

- New `snapshot` subcommand to `light-node` to produce trimmed copies of the storage.
- Option to enable retrying block application with cache reloaded.
- Implemented shell-side prechecker for endorsement operations.
- Implemented optional shell-side block prechecking.
- Implemented statistics RPCs for tracking new blocks processing.
- Re-implemented block application logic in state machine.
- Re-implemented protocol-runner subprocess handling in the state machine.

### Changed

- Rust upgraded to 1.58.1

### Fixed

- Missed endorsement when alternative heads are encountered.
- The `/monitor/bootstrapped` RPC now properly reports bootstrapping progress if the  node is not bootstrapped already.

## [1.15.1] - 2022-02-18

### Fixed

- Bug when mapping OCaml->Rust values representing error responses from protocol RPCs.

## [1.15.0] - 2022-02-18

### Added

- Various tools for the Rust implementation of the context storage (see `tezos/context-tool`).
- `context-integrity-check` flag to check the integrity of the Rust implementatino of the context storage at startup.

### Changed

- Released binaries no longer make use of ADX instructions, increasing comptability with more CPUs.

### Performance

- Improved the representation of the context storage inodes so that less memory is used.

## [1.14.0] - 2021-12-24

### Fixed

- In Rust implementation of persisted context storage, commits now
  behave as an atomic operations.

### Changed

- Switch default Rust toolchain to stable **Rust 1.57** version.
- Rewrote and moved mempool implementation from actor system to new state
  machine architecture.

### Added

- Compatibility with Hangzhou protocol.
- Rpc to get accumulated mempool operation statistics.

### Performance

- Optimized Rust implementation of persisted context storage.

## [1.13.0] - 2021-12-01

### Fixed

- Removed redundant validations of `current_head` from peers, which in some cases was causing the node to lag behind.

## [1.12.0] - 2021-11-30

### Changed

- Increase Irmin's index log size limit to `2_500_000` to match Octez v11. This should help with freezes during merges by making them happen less often, but increases memory usage a little.
- Tweak the call to apply a block in chain feeder so that it is less prone to block Tokio's worker threads.
- MessagePack encoding is now used for action and state snapshot storage instead of JSON.

## [1.11.0] - 2021-11-29

### Added

- Persistent on-disk backend for the TezEdge context storage (preview release)
- Enabling conditions for actions in shell automaton.
- Drone CI Pipeline with Python tests for Granada
- Per-protocol context statistics

### Removed

- Drone CI pipeline with Python tests for Edo

### Changed

- When using the TezEdge context storage implementation, the default backend is now the persistent one (was in-memory)
- More eager cleanup of no-longer used IPC connections between the shell process and the protocol runner process, and more reuse of already existing connections (instead of instantiation a new one each time) when possible.

### Fixed

- Issue that caused the list of peers between the state machine and the actors system to get out of sync, causing the node to lag behind.

## [1.10.0] - 2021-11-16

### Changed

- Rewrote P2P networking and peer management to new architecture.
- Made IPC communication with the protocol runner processes asynchronous.
- Renamed cli argument `--disable-peer-blacklist` to `--disable-peer-graylist`.

### Deprecated

- Synchronous ocaml IPC.

### Removed

- Actor based P2P networking and peer management.
- FFI connection pool flags:
  - `--ffi-pool-max-connections`
  - `--ffi-trpap-pool-max-connections`
  - `--ffi-twcap-pool-max-connections`
  - `--ffi-pool-connection-timeout-in-secs`
  - `--ffi-trpap-pool-connection-timeout-in-secs`
  - `--ffi-twcap-pool-connection-timeout-in-secs`
  - `--ffi-pool-max-lifetime-in-secs`
  - `--ffi-trpap-pool-max-lifetime-in-secs`
  - `--ffi-twcap-pool-max-lifetime-in-secs`
  - `--ffi-pool-idle-timeout-in-secs`
  - `--ffi-trpap-pool-idle-timeout-in-secs`
  - `--ffi-twcap-pool-idle-timeout-in-secs`

## [1.9.1] - 2021-11-04

### Fixed

- Optimized mempool operations downloading from p2p

## [1.9.0] - 2021-10-26

### Added

- Support for Ubuntu 21

### Changed

- Shell refactor and simplify communication between actors
- Upgrade to Tokio 1.12
- `riker` dependency replaced with `tezedge-actor-system` dependency

### Removed

- Removed `riker` dependency from `rpc` module

### Fixed

- Optimize `chains/{chain_id}/mempool/monitor_operations` and `monitor/heads/{chain_id}` RPCs.
- Controlled startup for chain_manager - run p2p only after ChainManager is subscribed to NetworkChannel
- ChainFeeder block application improved error handling with retry policy on protocol-runner restart
- Added set_size_hint after decoding read_message to avoid unnecessary recounting for websocket monitoring

## [1.8.0] - 2021-09-20

### Added

- Added new, faster implementation of binary encoding of p2p messages
- Added encoding benchmarks
- Added new context storage optimization that takes advantage of repeated directory structures
- Added build file cache to CI
- Added new, faster implementation for endorsing and baking rights RPCs that use cached snapshots data
- Added handlers for protocol 009 and 010 baking and endorsing rights RPC.

### Changed

- Changed historic protocols to use new storages for baking/endorsing rights calculation.
- Internal cleanup of the context storage implementation + documentation
- Replaced use of the failure crate with thiserror + anyhow crates

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

[Unreleased]: https://github.com/tezedge/tezedge/compare/v1.16.1...develop
[1.16.1]: https://github.com/tezedge/tezedge/releases/v1.16.1
[1.16.0]: https://github.com/tezedge/tezedge/releases/v1.16.0
[1.15.1]: https://github.com/tezedge/tezedge/releases/v1.15.1
[1.15.0]: https://github.com/tezedge/tezedge/releases/v1.15.0
[1.14.0]: https://github.com/tezedge/tezedge/releases/v1.14.0
[1.13.0]: https://github.com/tezedge/tezedge/releases/v1.13.0
[1.12.0]: https://github.com/tezedge/tezedge/releases/v1.12.0
[1.11.0]: https://github.com/tezedge/tezedge/releases/v1.11.0
[1.10.0]: https://github.com/tezedge/tezedge/releases/v1.10.0
[1.9.1]: https://github.com/tezedge/tezedge/releases/v1.9.1
[1.9.0]: https://github.com/tezedge/tezedge/releases/v1.9.0
[1.8.0]: https://github.com/tezedge/tezedge/releases/v1.8.0
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
