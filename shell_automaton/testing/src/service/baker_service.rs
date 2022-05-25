// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::mpsc;

use crypto::hash::{ChainId, Signature};
use shell_automaton::baker::block_endorser::{
    EndorsementWithForgedBytes, PreendorsementWithForgedBytes,
};
use shell_automaton::request::RequestId;
use shell_automaton::service::baker_service::{BakerService, BakerWorkerMessage, EncodeError};
use tezos_messages::base::signature_public_key::SignaturePublicKey;
use tezos_messages::p2p::encoding::block_header::BlockHeader;
use tezos_messages::protocol::proto_012::operation::FullHeader;

/// Mocked BakerService.
///
/// Does nothing.
#[derive(Debug, Clone)]
pub struct BakerServiceDummy {}

impl BakerServiceDummy {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for BakerServiceDummy {
    fn default() -> Self {
        Self::new()
    }
}

impl BakerService for BakerServiceDummy {
    fn try_recv(&mut self) -> Result<(RequestId, BakerWorkerMessage), mpsc::TryRecvError> {
        Err(mpsc::TryRecvError::Empty)
    }

    fn preendrosement_sign(
        &mut self,
        _baker: &SignaturePublicKey,
        _chain_id: &ChainId,
        _operation: &PreendorsementWithForgedBytes,
    ) -> Result<Signature, EncodeError> {
        Ok(Signature(vec![]))
    }

    fn endrosement_sign(
        &mut self,
        _baker: &SignaturePublicKey,
        _chain_id: &ChainId,
        _operation: &EndorsementWithForgedBytes,
    ) -> Result<Signature, EncodeError> {
        Ok(Signature(vec![]))
    }

    fn block_sign(
        &mut self,
        _baker: &SignaturePublicKey,
        _chain_id: &ChainId,
        _block_header: &BlockHeader,
    ) -> Result<Signature, EncodeError> {
        Ok(Signature(vec![]))
    }

    fn compute_proof_of_work(
        &mut self,
        baker: SignaturePublicKey,
        header: FullHeader,
        proof_of_work_threshold: u64,
    ) -> RequestId {
        RequestId::new_unchecked(0, 0)
    }
}
