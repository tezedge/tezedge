# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Mempool suppport

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

## [0.0.2] - 2020-06-01

### Added

- Support for connect to Carthagenet/Mainnet
- Support for Ubuntu 20 and OpenSUSE Tumbleweed
- RPC's for indexer blockwatch/tzindex (with drone integration test, which compares indexed data with Ocaml node against Tezedge node)
- Flags `--store-context-actions=BOOL.` If this flag is set to false, the node will persist less data to disk, which increases runtime speed.

### Changed

- P2p speed-up bootstrap - support for p2p_version 1 feature Nack_with_list, extended Nack - with potential peers to connect

### Removed

- Storing all p2p messages (moved to tezedge-debugger), the node will persist less data to disk

### Fixed / Security

- Remove bitvec dependency
- Refactored FFI to Ocaml not using BigArray1's for better GC processing
 
## [0.0.1] - 2020-03-31

### Added

- P2P Explorer support with dedicated RPC exposed
- Expose RPC for Tezos indexers
- Ability to connect and bootstrap data from Tezos Babylonnet
- Protocol FFI integration

[Unreleased]: https://github.com/simplestaking/tezedge/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/simplestaking/tezedge/releases/tag/v0.0.1

___
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
