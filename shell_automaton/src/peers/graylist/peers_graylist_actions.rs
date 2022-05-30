// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::{IpAddr, SocketAddr};

use serde::{Deserialize, Serialize};

use tezos_messages::p2p::encoding::ack::NackMotive;

use crate::peer::message::{read::PeerMessageReadError, write::PeerMessageWriteError};
use crate::{EnablingCondition, State};

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

    NackReceived(NackMotive),
    NackSent(NackMotive),

    HandshakeError,

    MessageReadError(PeerMessageReadError),
    MessageWriteError(PeerMessageWriteError),

    RequestedBlockHeaderLevelMismatch,

    BootstrapBlockHeaderInconsistentChain,
    BootstrapCementedBlockReorg,

    ConnectionClosed,
    Unknown,
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
