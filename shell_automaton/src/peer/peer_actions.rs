// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use storage::BlockHeaderWithHash;

use crate::{EnablingCondition, State};

use super::PeerIOLoopResult;
#[cfg(feature = "fuzzing")]
use crate::fuzzing::net::SocketAddrMutator;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerTryWriteLoopStartAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerTryWriteLoopStartAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerTryWriteLoopFinishAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub result: PeerIOLoopResult,
}

impl EnablingCondition<State> for PeerTryWriteLoopFinishAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerTryReadLoopStartAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerTryReadLoopStartAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerTryReadLoopFinishAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub result: PeerIOLoopResult,
}

impl EnablingCondition<State> for PeerTryReadLoopFinishAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerCurrentHeadUpdateAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub current_head: BlockHeaderWithHash,
}

impl EnablingCondition<State> for PeerCurrentHeadUpdateAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .filter(|p| {
                p.current_head
                    .as_ref()
                    .map(
                        |head| match self.current_head.header.level().cmp(&head.header.level()) {
                            std::cmp::Ordering::Equal => self.current_head.hash != head.hash,
                            std::cmp::Ordering::Greater => true,
                            std::cmp::Ordering::Less => false,
                        },
                    )
                    .unwrap_or(true)
            })
            .is_some()
    }
}
