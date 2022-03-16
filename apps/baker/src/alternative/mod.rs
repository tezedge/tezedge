// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use super::{key::CryptoService, proof_of_work::guess_proof_of_work};

mod event;

mod block_payload;

mod slots_info;

mod client;

mod timer;

mod event_loop;

pub use self::event_loop::run;
