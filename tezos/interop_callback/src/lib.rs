// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This crate exposes Rust FFI interface which can be called from the OCaml.
mod callback;

pub use callback::initialize_callbacks;