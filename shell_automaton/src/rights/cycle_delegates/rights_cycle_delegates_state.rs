// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use tezos_messages::base::signature_public_key::{SignaturePublicKey, SignaturePublicKeyHash};
use tezos_protocol_ipc_client::ProtocolServiceError;

use crate::{
    protocol_runner::ProtocolRunnerToken, service::protocol_runner_service::CycleDelegatesError,
    storage::kv_cycle_meta::Cycle,
};

pub type Delegates = BTreeMap<SignaturePublicKeyHash, SignaturePublicKey>;

pub type CycleDelegatesState = BTreeMap<Cycle, CycleDelegatesQuery>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct CycleDelegatesQuery {
    pub state: CycleDelegatesQueryState,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub enum CycleDelegatesQueryState {
    Init,
    ContextRequested(ProtocolRunnerToken),
    Success(Delegates),
    Error(DelegatesError),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub enum DelegatesError {
    #[error("Cannot communicate to protocol runner: {0}")]
    Protocol(#[from] ProtocolServiceError),
    #[error("Error response from protocol runner: {0}")]
    Response(#[from] CycleDelegatesError),
    #[error("Cannot decode cycle eras data: {0}")]
    Decode(String),
}
