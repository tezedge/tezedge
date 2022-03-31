// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use super::{key::CryptoService, proof_of_work::guess_proof_of_work};

pub mod event;

mod slots_info;

pub mod client;

pub mod timer;

mod event_loop;

mod seed_nonce;

pub use self::event_loop::run;
