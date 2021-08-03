// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

pub mod client;

pub fn force_libtezos_linking() {
    tezos_interop::force_libtezos_linking();
}
