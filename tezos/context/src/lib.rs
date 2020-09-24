// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! This crate contains code which is used to move context messages between OCaml and Rust worlds.
//!
//! Code in this crate should not reference any other tezedge crates to avoid circular dependencies.
//! At OCaml side message is pushed into crossbeam channel. At Rust side messages are fetched from
//! the crossbeam channel.

pub mod channel;
