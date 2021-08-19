// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use storage::BlockHeaderWithHash;
use tezos_messages::base::fitness_comparator::*;
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_messages::p2p::encoding::prelude::BlockHeader;
use tezos_messages::Head;

/// This struct holds info about local and remote "current" head
#[derive(Clone, Debug)]
pub struct CurrentHead {
    /// Represents local current head. Value here is the same as the
    /// hash of the last applied block.
    local: Option<Head>,
    /// Remote current head. This represents info about the best known remote head.
    remote: Option<Head>,
}

impl CurrentHead {
    pub fn new(local: Option<Head>, remote: Option<Head>) -> Self {
        Self { local, remote }
    }

    /// Validate if we can accept branch
    /// Returns true, if we need to process that branch
    pub fn accept_remote_branch(
        &mut self,
        block_header: &BlockHeader,
        block_header_head: Head,
    ) -> bool {
        // ignore genesis
        if block_header.level() <= 0 {
            return false;
        }

        // lets check our actual local head
        if let Some(local_current_head) = self.local.as_ref() {
            // (only_if_fitness_increases) we can accept branch if increases fitness
            if fitness_increases(local_current_head.fitness(), block_header.fitness()) {
                self.remote = Some(block_header_head);
                true
            } else {
                false
            }
        } else {
            // no local head, we can accept received branch
            self.remote = Some(block_header_head);
            true
        }
    }
}
