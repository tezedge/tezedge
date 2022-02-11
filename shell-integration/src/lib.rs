// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

pub use connector::*;
pub use messages::*;
pub use oneshot::*;
pub use retry::*;
pub use streaming_state::*;

mod connector;
mod messages;
mod oneshot;
mod retry;
mod streaming_state;

#[derive(Debug)]
pub struct UnsupportedMessageError;

/// Dedicated for scenarios, where dont know how to handle error, only just to log error and propagate it
#[derive(Debug)]
pub struct UnexpectedError {
    pub reason: String,
}
