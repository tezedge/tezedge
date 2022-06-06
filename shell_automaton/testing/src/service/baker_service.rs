// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{ChainId, Signature};
use shell_automaton::baker::block_endorser::{
    EndorsementWithForgedBytes, PreendorsementWithForgedBytes,
};
use shell_automaton::request::RequestId;
use shell_automaton::service::baker_service::{BakerService, BakerWorkerMessage, EncodeError};
use shell_automaton::service::service_channel::ResponseTryRecvError;
use tezos_messages::base::signature_public_key::SignaturePublicKey;
use tezos_messages::p2p::encoding::block_header::BlockHeader;

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
    fn try_recv(&mut self) -> Result<(RequestId, BakerWorkerMessage), ResponseTryRecvError> {
        Err(ResponseTryRecvError::Empty)
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
        _baker: SignaturePublicKey,
        _header: BlockHeader,
        _proof_of_work_threshold: u64,
    ) -> RequestId {
        RequestId::new_unchecked(0, 0)
    }
}
