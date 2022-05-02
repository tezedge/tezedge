// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

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
