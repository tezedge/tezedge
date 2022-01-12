// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod action;
pub mod service;
pub mod state;

mod reducer;
pub use self::reducer::reducer;

mod effects;
pub use self::effects::effects;
