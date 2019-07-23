Tezos-rs
===========

The purpose of this project is to implement a secure, trustworthy, open-source Tezos node in Rust. In addition to implementing a new node, the project seeks to maintain and improve the Tezos node wherever possible. 

The project can be considered as a proof of concept. This PoC demonstrates the viability of a node build that uses Rust. 

## Components

* **light_node:** Implementation of a lightweight Tezos node written in Rust.
  If you want to run node, then enter *light_node* directory and then execute `cargo run`. For more details consult node [README.md](light_node/README.md) file.

* **tezos_encoding:** All incoming messages are transformed into standard Rust structures for easy manipulation using de component. This component implements serialization and deserialization of all data types used in Tezos messages.

* **crypto:** Component contains cryptographic algorithms for encryption and decryption of messages.  


