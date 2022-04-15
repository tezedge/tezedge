// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{btree_map, BTreeMap};

use crypto::hash::{BlockHash, ProtocolHash};
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_messages::p2p::encoding::block_header::BlockHeader;
use tezos_protocol_ipc_client::ProtocolServiceError;

use crate::{
    protocol_runner::ProtocolRunnerToken, service::protocol_runner_service::ContextRawBytesError,
    storage::kv_cycle_eras,
};

use super::CycleEras;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CycleErasState(pub(super) BTreeMap<ProtocolHash, CycleErasQuery>);

impl CycleErasState {
    pub(crate) fn get_result(&self, key: &ProtocolHash) -> Option<&CycleEras> {
        if let Some(CycleErasQueryState::Success(cycle_eras)) = self.get_state(key) {
            Some(cycle_eras)
        } else {
            None
        }
    }

    pub(crate) fn get_error(&self, key: &ProtocolHash) -> Option<&CycleErasError> {
        if let Some(CycleErasQueryState::Error(error)) = self.get_state(key) {
            Some(error)
        } else {
            None
        }
    }

    pub(super) fn contains_key(&self, key: &ProtocolHash) -> bool {
        self.0.contains_key(key)
    }

    pub(super) fn insert(&mut self, key: ProtocolHash, value: CycleErasQuery) {
        self.0.insert(key, value);
    }

    pub(super) fn get(&self, key: &ProtocolHash) -> Option<&CycleErasQuery> {
        self.0.get(key)
    }

    pub(super) fn get_mut(&mut self, key: &ProtocolHash) -> Option<&mut CycleErasQuery> {
        self.0.get_mut(key)
    }

    pub(super) fn get_state(&self, key: &ProtocolHash) -> Option<&CycleErasQueryState> {
        self.get(key).map(|q| &q.state)
    }

    pub(super) fn get_state_mut(&mut self, key: &ProtocolHash) -> Option<&mut CycleErasQueryState> {
        self.get_mut(key).map(|q| &mut q.state)
    }

    pub(super) fn iter(&self) -> btree_map::Iter<ProtocolHash, CycleErasQuery> {
        self.0.iter()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CycleErasQuery {
    pub block_hash: BlockHash,
    pub block_header: BlockHeader,
    pub state: CycleErasQueryState,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CycleErasQueryState {
    PendingKV,
    PendingContext(KvStoreError),
    ContextRequested(ProtocolRunnerToken, KvStoreError),
    Success(CycleEras),
    Error(CycleErasError),
}

pub type KvStoreError = kv_cycle_eras::Error;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub enum ContextError {
    #[error("Cannot communicate to protocol runner: {0}")]
    Protocol(#[from] ProtocolServiceError),
    #[error("Error response from protocol runner: {0}")]
    Response(#[from] ContextRawBytesError),
    #[error("Cannot decode cycle eras data: {0}")]
    Decode(String),
}

impl From<BinaryReaderError> for ContextError {
    fn from(error: BinaryReaderError) -> Self {
        Self::Decode(error.to_string())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[error("kv store error: {0}, context request error: {1}")]
pub struct CycleErasError(KvStoreError, ContextError);

impl CycleErasError {
    pub(super) fn new(kv_store_error: KvStoreError, context_error: ContextError) -> Self {
        Self(kv_store_error, context_error)
    }
}
