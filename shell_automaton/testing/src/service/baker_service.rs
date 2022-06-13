// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{ChainId, SecretKeyEd25519};
use shell_automaton::baker::block_endorser::{
    EndorsementWithForgedBytes, PreendorsementWithForgedBytes,
};
use shell_automaton::request::RequestId;
use shell_automaton::service::baker_service::{BakerService, BakerWorkerMessage};
use shell_automaton::service::service_channel::ResponseTryRecvError;
use tezos_messages::base::signature_public_key::SignaturePublicKeyHash;
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
    fn add_local_baker(&mut self, _pkh: SignaturePublicKeyHash, _secret_key: SecretKeyEd25519) {}

    fn try_recv(&mut self) -> Result<(RequestId, BakerWorkerMessage), ResponseTryRecvError> {
        Err(ResponseTryRecvError::Empty)
    }

    fn preendrosement_sign(
        &mut self,
        _baker: &SignaturePublicKeyHash,
        _chain_id: &ChainId,
        _operation: &PreendorsementWithForgedBytes,
    ) -> RequestId {
        RequestId::new_unchecked(0, 0)
    }

    fn endrosement_sign(
        &mut self,
        _baker: &SignaturePublicKeyHash,
        _chain_id: &ChainId,
        _operation: &EndorsementWithForgedBytes,
    ) -> RequestId {
        RequestId::new_unchecked(0, 0)
    }

    fn block_sign(
        &mut self,
        _baker: &SignaturePublicKeyHash,
        _chain_id: &ChainId,
        _block_header: &BlockHeader,
    ) -> RequestId {
        RequestId::new_unchecked(0, 0)
    }

    fn compute_proof_of_work(
        &mut self,
        _baker: SignaturePublicKeyHash,
        _header: BlockHeader,
        _proof_of_work_threshold: i64,
    ) -> RequestId {
        RequestId::new_unchecked(0, 0)
    }
}
