// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
// #![forbid(unsafe_code)]

pub mod environment;
pub mod ffi;
pub mod ocaml_conv;
mod octez_config;

#[cfg(feature = "tezos-sys")]
pub fn force_libtezos_linking() {
    tezos_sys::force_libtezos_linking();
}
