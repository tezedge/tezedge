// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use super::proof_of_work::guess_proof_of_work;

mod slots_info;

mod event_loop;

mod seed_nonce;

pub use self::event_loop::run;
