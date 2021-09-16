// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crypto::hash::ContextHash;
use tezos_context_api::{ContextKeyOwned, ContextValue, StringTreeObject};
use tezos_wrapper::TezosApiConnectionPool;
use thiserror::Error;

#[derive(Clone)]
pub struct TezedgeContextClient {
    tezos_readonly_api: Arc<TezosApiConnectionPool>,
}

#[derive(Debug, Error)]
pub enum TezedgeContextClientError {
    #[error("Pool error: {error:?}")]
    PoolError {
        error: tezos_wrapper::InternalPoolError,
    },
    #[error("Protocol service error: {error:?}")]
    ProtocolServiceError {
        error: tezos_wrapper::service::ProtocolServiceError,
    },
}

impl From<tezos_wrapper::InternalPoolError> for TezedgeContextClientError {
    fn from(error: tezos_wrapper::InternalPoolError) -> Self {
        TezedgeContextClientError::PoolError { error }
    }
}

impl From<tezos_wrapper::service::ProtocolServiceError> for TezedgeContextClientError {
    fn from(error: tezos_wrapper::service::ProtocolServiceError) -> Self {
        TezedgeContextClientError::ProtocolServiceError { error }
    }
}

impl TezedgeContextClient {
    pub fn new(tezos_readonly_api: Arc<TezosApiConnectionPool>) -> Self {
        Self { tezos_readonly_api }
    }

    pub fn get_key_from_history(
        &self,
        context_hash: &ContextHash,
        key: ContextKeyOwned,
    ) -> Result<Option<ContextValue>, TezedgeContextClientError> {
        Ok(self
            .tezos_readonly_api
            .pool
            .get()?
            .api
            .get_context_key_from_history(context_hash, key)?)
    }

    pub fn get_key_values_by_prefix(
        &self,
        context_hash: &ContextHash,
        prefix: ContextKeyOwned,
    ) -> Result<Option<Vec<(ContextKeyOwned, ContextValue)>>, TezedgeContextClientError> {
        Ok(self
            .tezos_readonly_api
            .pool
            .get()?
            .api
            .get_context_key_values_by_prefix(context_hash, prefix)?)
    }

    pub fn get_context_tree_by_prefix(
        &self,
        context_hash: &ContextHash,
        prefix: ContextKeyOwned,
        depth: Option<usize>,
    ) -> Result<StringTreeObject, TezedgeContextClientError> {
        Ok(self
            .tezos_readonly_api
            .pool
            .get()?
            .api
            .get_context_tree_by_prefix(context_hash, prefix, depth)?)
    }
}
