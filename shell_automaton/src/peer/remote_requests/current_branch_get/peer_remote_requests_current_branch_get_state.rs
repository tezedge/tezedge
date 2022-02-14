// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;
use crypto::seeded_step::Step;
use tezos_messages::p2p::encoding::block_header::{BlockHeader, Level};
use tezos_messages::p2p::encoding::prelude::CurrentBranch;

use crate::request::RequestId;
use crate::service::storage_service::StorageError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerRemoteRequestsCurrentBranchGetNextBlockState {
    Idle {},
    Init {
        level: Level,
    },
    Pending {
        level: Level,
        storage_req_id: RequestId,
    },
    Error {
        level: Level,
        error: StorageError,
    },
    Success {
        level: Level,
        result: Option<BlockHash>,
    },
}

impl PeerRemoteRequestsCurrentBranchGetNextBlockState {
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle { .. })
    }

    pub fn is_init(&self) -> bool {
        matches!(self, Self::Init { .. })
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Storage request successful but block hash not found for level.
    pub fn is_empty_success(&self) -> bool {
        match self {
            Self::Success { result, .. } => result.is_none(),
            _ => false,
        }
    }

    pub fn storage_req_id(&self) -> Option<RequestId> {
        match self {
            Self::Pending { storage_req_id, .. } => Some(*storage_req_id),
            _ => None,
        }
    }

    pub fn level(&self) -> Option<Level> {
        match self {
            Self::Idle { .. } => None,
            Self::Init { level, .. }
            | Self::Pending { level, .. }
            | Self::Error { level, .. }
            | Self::Success { level, .. } => Some(*level),
        }
    }

    pub fn init(&mut self, level: Level) {
        *self = Self::Init { level };
    }

    pub fn to_pending(&mut self, storage_req_id: RequestId) {
        let level = match self.level() {
            Some(v) => v,
            None => return,
        };
        *self = Self::Pending {
            level,
            storage_req_id,
        };
    }

    pub fn to_error(&mut self, error: StorageError) {
        let level = match self.level() {
            Some(v) => v,
            None => return,
        };
        *self = Self::Error { level, error };
    }

    pub fn to_success(&mut self, result: Option<BlockHash>) {
        let level = match self.level() {
            Some(v) => v,
            None => return,
        };
        *self = Self::Success { level, result };
    }
}

impl Default for PeerRemoteRequestsCurrentBranchGetNextBlockState {
    fn default() -> Self {
        Self::Idle {}
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerRemoteRequestsCurrentBranchGetState {
    Idle {
        time: u64,
    },
    Init {
        time: u64,
    },
    Pending {
        current_head: BlockHeader,
        history: Vec<BlockHash>,
        step: Step,
        next_block: PeerRemoteRequestsCurrentBranchGetNextBlockState,
    },
    Success {
        result: CurrentBranch,
    },
}

impl PeerRemoteRequestsCurrentBranchGetState {
    pub fn is_init(&self) -> bool {
        matches!(self, Self::Init { .. })
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    /// Is state ready to transition from `Pending` to `Success`.
    pub fn is_complete(&self) -> bool {
        match self {
            Self::Pending {
                current_head,
                step,
                next_block,
                ..
            } => {
                let next_level = next_block.level().unwrap_or(current_head.level());
                next_block.is_empty_success()
                    || next_block.is_error()
                    || next_level < step.clone().next_step().max(1)
            }
            _ => false,
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    pub fn next_block(&self) -> Option<&PeerRemoteRequestsCurrentBranchGetNextBlockState> {
        match self {
            Self::Pending { next_block, .. } => Some(next_block),
            _ => None,
        }
    }

    pub fn next_block_mut(
        &mut self,
    ) -> Option<&mut PeerRemoteRequestsCurrentBranchGetNextBlockState> {
        match self {
            Self::Pending { next_block, .. } => Some(next_block),
            _ => None,
        }
    }

    pub fn next_block_is_idle_or_success(&self) -> bool {
        self.next_block()
            .filter(|v| v.is_idle() || v.is_success())
            .is_some()
    }

    pub fn storage_req_id(&self) -> Option<RequestId> {
        self.next_block().and_then(|v| v.storage_req_id())
    }

    pub fn step_next_level(&mut self) -> Option<Level> {
        match self {
            Self::Pending {
                current_head,
                step,
                next_block,
                ..
            } => {
                let level = next_block.level().unwrap_or(current_head.level());
                let step = step.next_step();
                if level < step {
                    None
                } else {
                    Some(level - step)
                }
            }
            _ => None,
        }
    }
}

impl Default for PeerRemoteRequestsCurrentBranchGetState {
    fn default() -> Self {
        Self::Idle { time: 0 }
    }
}
