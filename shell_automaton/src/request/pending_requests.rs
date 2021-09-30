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
}

impl<Request> PendingRequests<Request> {
    pub fn new() -> Self {
        Self {
            list: Slab::new(),
            counter: 0,
            last_added_req_id: RequestId::new(0, 0),
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

        req_id
    }

    #[inline]
    pub fn remove(&mut self, id: RequestId) -> Option<Request> {
        if self.get(id).is_none() {
            return None;
        }
        Some(self.list.remove(id.locator()).request)
    }
}
