// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod event;

mod block_payload;

mod slots_info;

mod client;

mod timer;

mod event_loop;

pub use self::event_loop::run;
