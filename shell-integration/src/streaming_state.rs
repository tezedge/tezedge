// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use futures::task::Waker;
use uuid::Uuid;

pub type StreamWakers = HashMap<Uuid, Waker>;

pub trait StreamCounter {
    fn get_mutable_streams(&mut self) -> &mut StreamWakers;
    fn get_streams(&self) -> StreamWakers;

    fn add_stream(&mut self, id: Uuid, waker: Waker) {
        let streams = self.get_mutable_streams();

        streams.insert(id, waker);
    }
    fn remove_stream(&mut self, id: Uuid) {
        let streams = self.get_mutable_streams();

        streams.remove(&id);
    }
    fn wake_up_all_streams(&self) {
        let streams = self.get_streams();

        streams.iter().for_each(|(_, waker)| waker.wake_by_ref())
    }
}
