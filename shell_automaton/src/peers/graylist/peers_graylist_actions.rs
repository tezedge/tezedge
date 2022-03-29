// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};

use crate::{peer::message::write::PeerMessageWriteError, EnablingCondition, State};

#[cfg(feature = "fuzzing")]
use crate::fuzzing::net::{IpAddrMutator, SocketAddrMutator};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerGraylistReason {
    ConnectionIncomingError,
    ConnectionOutgoingError,

    BinaryMessageReadError,
    BinaryMessageWriteError,

    ChunkReadError,
    ChunkWriteError,

    NackReceived,
    NackSent,

    HandshakeError,

    MessageReadError,
    MessageWriteError(PeerMessageWriteError),

    RequestedBlockHeaderLevelMismatch,

    BootstrapBlockHeaderInconsistentChain,
    BootstrapCementedBlockReorg,

    ConnectionClosed,
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistAddressAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,

    pub reason: PeerGraylistReason,
}

impl EnablingCondition<State> for PeersGraylistAddressAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistIpAddAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(IpAddrMutator))]
    pub ip: IpAddr,
}

impl EnablingCondition<State> for PeersGraylistIpAddAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistIpAddedAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(IpAddrMutator))]
    pub ip: IpAddr,
}

impl EnablingCondition<State> for PeersGraylistIpAddedAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistIpRemoveAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(IpAddrMutator))]
    pub ip: IpAddr,
}

impl EnablingCondition<State> for PeersGraylistIpRemoveAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistIpRemovedAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(IpAddrMutator))]
    pub ip: IpAddr,
}

impl EnablingCondition<State> for PeersGraylistIpRemovedAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
