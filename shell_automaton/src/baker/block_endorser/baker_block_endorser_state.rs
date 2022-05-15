// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, HashType, OperationHash, Signature};
use tezos_encoding::enc::{BinError, BinWriter};
use tezos_messages::protocol::proto_012::operation::{
    InlinedEndorsementMempoolContents, InlinedPreendorsementContents,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationWithForgedBytes<T> {
    branch: BlockHash,
    operation: T,
    forged: Vec<u8>,
}

impl<T: BinWriter> OperationWithForgedBytes<T> {
    pub fn new(branch: BlockHash, operation: T) -> Result<Self, BinError> {
        let mut forged = vec![];
        branch.bin_write(&mut forged)?;
        operation.bin_write(&mut forged)?;
        Ok(Self {
            branch,
            operation,
            forged,
        })
    }

    pub fn branch(&self) -> &BlockHash {
        &self.branch
    }

    pub fn operation(&self) -> &T {
        &self.operation
    }

    pub fn forged(&self) -> &[u8] {
        &self.forged[HashType::BlockHash.size()..]
    }

    pub fn forged_with_branch(&self) -> &[u8] {
        &self.forged
    }
}

pub type PreendorsementWithForgedBytes = OperationWithForgedBytes<InlinedPreendorsementContents>;
pub type EndorsementWithForgedBytes = OperationWithForgedBytes<InlinedEndorsementMempoolContents>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BakerBlockEndorserState {
    Idle {
        time: u64,
    },
    RightsGetPending {
        time: u64,
    },
    RightsGetSuccess {
        time: u64,
        /// First slot for current block.
        first_slot: u16,
    },
    NoRights {
        time: u64,
    },
    /// Payload is older than the last locked payload.
    PayloadOutdated {
        time: u64,
        first_slot: u16,
    },
    /// Current head is the same level as the last endorsed block, but
    /// payload hash is different. We need to wait for prequorum.
    PayloadLocked {
        time: u64,
        first_slot: u16,
    },
    /// Payload was locked, but prequorum was reached for the new
    /// payload hash so unlock it to preendorse and endorse the payload.
    PayloadUnlockedAsPreQuorumReached {
        time: u64,
        first_slot: u16,
    },
    Preendorse {
        time: u64,
        first_slot: u16,
    },
    PreendorsementSignPending {
        time: u64,
        first_slot: u16,
        operation: PreendorsementWithForgedBytes,
    },
    PreendorsementSignSuccess {
        time: u64,
        first_slot: u16,
        operation: PreendorsementWithForgedBytes,
        signature: Signature,
    },
    PreendorsementInjectPending {
        time: u64,
        first_slot: u16,
        operation_hash: OperationHash,
        operation: PreendorsementWithForgedBytes,
        signature: Signature,
        signed_operation_bytes: Vec<u8>,
    },
    PreendorsementInjectSuccess {
        time: u64,
        first_slot: u16,
        operation_hash: OperationHash,
        operation: PreendorsementWithForgedBytes,
        signature: Signature,
        signed_operation_bytes: Vec<u8>,
    },
    PreQuorumPending {
        time: u64,
        first_slot: u16,
    },
    PreQuorumSuccess {
        time: u64,
        first_slot: u16,
    },
    Endorse {
        time: u64,
        first_slot: u16,
    },
    EndorsementSignPending {
        time: u64,
        first_slot: u16,
        operation: EndorsementWithForgedBytes,
    },
    EndorsementSignSuccess {
        time: u64,
        first_slot: u16,
        operation: EndorsementWithForgedBytes,
        signature: Signature,
    },
    EndorsementInjectPending {
        time: u64,
        first_slot: u16,
        operation_hash: OperationHash,
        operation: EndorsementWithForgedBytes,
        signature: Signature,
        signed_operation_bytes: Vec<u8>,
    },
    EndorsementInjectSuccess {
        time: u64,
        first_slot: u16,
        operation_hash: OperationHash,
        operation: EndorsementWithForgedBytes,
        signature: Signature,
        signed_operation_bytes: Vec<u8>,
    },
}

impl BakerBlockEndorserState {
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle { .. })
    }

    pub fn first_slot(&self) -> Option<u16> {
        // TODO(zura)
        match self {
            Self::RightsGetSuccess { first_slot, .. }
            | Self::PayloadOutdated { first_slot, .. }
            | Self::PayloadLocked { first_slot, .. }
            | Self::PayloadUnlockedAsPreQuorumReached { first_slot, .. }
            | Self::Preendorse { first_slot, .. }
            | Self::PreendorsementSignPending { first_slot, .. }
            | Self::PreendorsementSignSuccess { first_slot, .. }
            | Self::PreendorsementInjectPending { first_slot, .. }
            | Self::PreendorsementInjectSuccess { first_slot, .. }
            | Self::PreQuorumPending { first_slot, .. }
            | Self::PreQuorumSuccess { first_slot, .. }
            | Self::Endorse { first_slot, .. }
            | Self::EndorsementSignPending { first_slot, .. }
            | Self::EndorsementSignSuccess { first_slot, .. }
            | Self::EndorsementInjectPending { first_slot, .. }
            | Self::EndorsementInjectSuccess { first_slot, .. } => Some(*first_slot),
            _ => return None,
        }
    }

    pub fn is_preendorsement_inject_pending(&self) -> bool {
        matches!(self, Self::PreendorsementInjectPending { .. })
    }

    pub fn is_endorsement_inject_pending(&self) -> bool {
        matches!(self, Self::EndorsementInjectPending { .. })
    }
}
