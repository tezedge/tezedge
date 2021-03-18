// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
// #![forbid(unsafe_code)]

pub mod environment;
pub mod ffi;
pub mod ocaml_conv;

pub fn force_libtezos_linking() {
    tezos_sys::force_libtezos_linking();
}
