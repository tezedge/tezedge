// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

mod connector;
pub use connector::*;

mod oneshot;
pub use oneshot::*;

mod messages;
pub use messages::*;

mod streaming_state;
pub use streaming_state::*;

#[derive(Debug)]
pub struct UnsupportedMessageError;

/// Dedicated for scenarios, where dont know how to handle error, only just to log error and propagate it
#[derive(Debug)]
pub struct UnexpectedError {
    pub reason: String,
}
