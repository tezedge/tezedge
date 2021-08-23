// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tla_sm::Proposal;

use crate::PeerAddress;

pub struct PeerHandshakeMessageProposal<'a, Efs, M> {
    pub effects: &'a mut Efs,
    pub peer: PeerAddress,
    pub message: M,
}

impl<'a, Efs, M> Proposal for PeerHandshakeMessageProposal<'a, Efs, M> {}
