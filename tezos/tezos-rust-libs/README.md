**Vendored from: https://gitlab.com/tezos/tezos-rust-libs/-/tree/v1.1**

# Tezos rust libraries (and dependencies)
This repository contains all the rust libraries used in the codebase of tezos/tezos as well as all their dependencies vendored. The purpose is to make a self-contained archive which allows a compilation inside `opam` "sandbox".

## How to change something
 - Add or update libraries
 - Complete or adapt the list in `Cargo.toml`
 - Refresh `Cargo.lock` with `cargo update`
 - Run `cargo vendor` to regenerate `vendor/`
 - Commit everything

