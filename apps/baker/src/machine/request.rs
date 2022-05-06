// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;

pub struct Request<Id, Ok, Err> {
    pub id: Id,
    pub state: RequestState<Ok, Err>,
}

pub enum RequestState<Ok, Err> {
    Pending,
    Success(Ok),
    Error(Err),
}

impl<Id, Ok, Err> Request<Id, Ok, Err> {
    pub fn new(id: Id) -> Self {
        Request {
            id,
            state: RequestState::Pending,
        }
    }

    pub fn is_pending(&self) -> bool {
        matches!(&self.state, RequestState::Pending)
    }

    pub fn done_ok(self, ok: Ok) -> Self {
        Request {
            id: self.id,
            state: RequestState::Success(ok),
        }
    }

    #[allow(dead_code)]
    pub fn done_err(self, err: Err) -> Self {
        Request {
            id: self.id,
            state: RequestState::Error(err),
        }
    }
}

impl<Id, Ok, Err> fmt::Display for Request<Id, Ok, Err>
where
    Id: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.state {
            RequestState::Pending => write!(f, "pending {}", self.id),
            RequestState::Success(_) => write!(f, "success {}", self.id),
            RequestState::Error(_) => write!(f, "error {}", self.id),
        }
    }
}
