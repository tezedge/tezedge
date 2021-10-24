use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::net::SocketAddr;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum YieldedOperation {
    PeerTryWriteLoop { peer_address: SocketAddr },
    PeerTryReadLoop { peer_address: SocketAddr },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum YieldedOperationCurrent {
    None,
    Init(YieldedOperation),
    Success,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct YieldedOperationsState {
    list: VecDeque<YieldedOperation>,
    pub(super) current: YieldedOperationCurrent,
}

impl YieldedOperationsState {
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            list: VecDeque::new(),
            current: YieldedOperationCurrent::None,
        }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    #[inline(always)]
    pub(super) fn add(&mut self, op: YieldedOperation) {
        self.list.push_back(op)
    }

    #[inline(always)]
    pub(super) fn pop_front(&mut self) -> Option<YieldedOperation> {
        self.list.pop_front()
    }
}
