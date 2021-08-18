// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt::Debug;

/// Get requests from state machine that need to be executed by some
/// entity managing the state machine.
///
/// Progress might need to be fed back to state machine depending on the request.
pub trait GetRequests {
    type Request: Debug;

    fn get_requests(&self, buf: &mut Vec<Self::Request>) -> usize;
}
