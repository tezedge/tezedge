// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use slab::Slab;

use super::RequestId;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PendingRequest<Request> {
    counter: usize,
    request: Request,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PendingRequests<Request> {
    list: Slab<PendingRequest<Request>>,
    counter: usize,
    last_added_req_id: RequestId,

    // TODO: Maybe extend slab to allow access to next_index value
    // with immutable borrow. At the moment only possible with
    // `Slab::vacant_entry().key()` but it required mutable access.
    next_index: usize,
}

impl<Request> PendingRequests<Request> {
    pub fn new() -> Self {
        Self {
            list: Slab::new(),
            counter: 0,
            last_added_req_id: RequestId::new(0, 0),
            next_index: 0,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    #[inline]
    pub fn last_added_req_id(&self) -> RequestId {
        self.last_added_req_id
    }

    #[inline]
    pub fn next_req_id(&self) -> RequestId {
        RequestId::new(self.next_index, self.counter.wrapping_add(1))
    }

    #[inline]
    pub fn contains(&self, id: RequestId) -> bool {
        self.get(id).is_some()
    }

    #[inline]
    pub fn get(&self, id: RequestId) -> Option<&Request> {
        self.list
            .get(id.locator())
            .filter(|req| req.counter == id.counter())
            .map(|x| &x.request)
    }

    #[inline]
    pub fn get_mut(&mut self, id: RequestId) -> Option<&mut Request> {
        self.list
            .get_mut(id.locator())
            .filter(|req| req.counter == id.counter())
            .map(|x| &mut x.request)
    }

    #[inline]
    pub fn add(&mut self, request: Request) -> RequestId {
        self.counter = self.counter.wrapping_add(1);

        let locator = self.list.insert(PendingRequest {
            counter: self.counter,
            request,
        });

        let req_id = RequestId::new(locator, self.counter);
        self.last_added_req_id = req_id;
        self.next_index = self.list.vacant_entry().key();

        req_id
    }

    #[inline]
    pub fn remove(&mut self, id: RequestId) -> Option<Request> {
        self.get(id)?;
        let removed_req = self.list.remove(id.locator()).request;
        self.next_index = self.list.vacant_entry().key();
        Some(removed_req)
    }

    pub fn iter<'a>(&'a self) -> impl 'a + Iterator<Item = (RequestId, &'a Request)> {
        self.list
            .iter()
            .map(|(locator, req)| (RequestId::new(locator, req.counter), &req.request))
    }
}
