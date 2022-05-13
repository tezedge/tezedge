// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{ChainId, Signature};
use shell_automaton::baker::block_endorser::{
    EndorsementWithForgedBytes, PreendorsementWithForgedBytes,
};
use shell_automaton::service::baker_service::{BakerService, EncodeError};
use tezos_messages::base::signature_public_key::SignaturePublicKey;

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
}
