Tezos-rs
===========

The purpose of this project is to implement a secure, trustworthy, open-source Tezos node in Rust. In addition to implementing a new node, the project seeks to maintain and improve the Tezos node wherever possible. 

The project can be considered as a proof of concept. This PoC demonstrates the viability of a node build that uses Rust. 

## Components

* **light_node:** Implementation of a lightweight Tezos node written in Rust.
  If you want to run node, then enter *light_node* directory and then execute `cargo run`. For more details consult node [README.md](light_node/README.md) file.

* **tezos_encoding:** All incoming messages are transformed into standard Rust structures for easy manipulation using de component. This component implements serialization and deserialization of all data types used in Tezos messages.

* **crypto:** Component contains cryptographic algorithms for encryption and decryption of messages.  


Requirements
------------

**1. Install Rust** 

We recommend installing Rust through rustup.

Run the following in your terminal, then follow the onscreen instructions.

```
curl https://sh.rustup.rs -sSf | sh
```

**2. Rust version** 

Rust nightly is required to build this project.
```
rustup install nightly-2019-07-30
rustup default nightly-2019-07-30
```
Application has been tested to compile with `rustc 1.38.0-nightly (dddb7fca0 2019-07-30)`.


Building
--------

**3. Build node** 

On linux systems:

```
export SODIUM_USE_PKG_CONFIG=1
cargo build
```

On windows systems with vcpkg:
```
set SODIUM_USE_PKG_CONFIG=1
set VCPKGRS_DYNAMIC=1
cargo build
```
