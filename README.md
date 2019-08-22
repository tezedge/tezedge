Tezos-rs
===========

The purpose of this project is to implement a secure, trustworthy, open-source Tezos node in Rust. In addition to implementing a new node, the project seeks to maintain and improve the Tezos node wherever possible. 

The project can be considered as a proof of concept. This PoC demonstrates the viability of a node build that uses Rust. 

## Components

* **light_node:** Implementation of a lightweight Tezos node written in Rust.
  If you want to run node, then enter *light_node* directory and then execute `cargo run`. For more details consult node [README](light_node/README.md) file.

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
rustup install nightly-2019-08-22
rustup default nightly-2019-08-22
```
Application has been tested to compile with `rustc 1.39.0-nightly (e44fdf979 2019-08-21)`.

**3. Install OCaml**

To build ocaml binaries used by our codebase you need to have opam installed.

On linux systems first download and install opam:

```
wget https://github.com/ocaml/opam/releases/download/2.0.5/opam-2.0.5-x86_64-linux
sudo cp opam-2.0.5-x86_64-linux /usr/local/bin/opam
sudo chmod a+x /usr/local/bin/opam
```

Then install required OCaml version and dune package manager:
```
opam switch create 4.07.0
opam switch set 4.07.0
opam update
opam install dune
```


Building
--------

**4. Build node** 

On linux systems:

```
export SODIUM_USE_PKG_CONFIG=1
cargo build
```

For more info on how to compile OCaml interop library please see [README](tezos_interop/README.md).
