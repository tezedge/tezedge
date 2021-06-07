// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crypto::hash::ContextHash;
use failure::Fail;
use tezos_new_context::{ContextError, ContextKeyOwned, ContextValue, StringTreeEntry};
use tezos_wrapper::TezosApiConnectionPool;

#[derive(Clone)]
pub struct TezedgeContextRemote {
    tezos_readonly_api: Arc<TezosApiConnectionPool>,
}

#[derive(Debug, Fail)]
pub enum RemoteContextError {
    #[fail(display = "Context error: {:?}", error)]
    ContextError { error: ContextError },
    #[fail(display = "Pool error: {:?}", error)]
    PoolError {
        error: tezos_wrapper::InternalPoolError,
    },
    #[fail(display = "Protocol service error: {:?}", error)]
    ProtocolServiceError {
        error: tezos_wrapper::service::ProtocolServiceError,
    },
}

impl From<ContextError> for RemoteContextError {
    fn from(error: ContextError) -> Self {
        RemoteContextError::ContextError { error }
    }
}

impl From<tezos_wrapper::InternalPoolError> for RemoteContextError {
    fn from(error: tezos_wrapper::InternalPoolError) -> Self {
        RemoteContextError::PoolError { error }
    }
}

impl From<tezos_wrapper::service::ProtocolServiceError> for RemoteContextError {
    fn from(error: tezos_wrapper::service::ProtocolServiceError) -> Self {
        RemoteContextError::ProtocolServiceError { error }
    }
}

impl TezedgeContextRemote {
    pub fn new(tezos_readonly_api: Arc<TezosApiConnectionPool>) -> Self {
        Self { tezos_readonly_api }
    }

    pub fn get_key_from_history(
        &self,
        context_hash: &ContextHash,
        key: ContextKeyOwned,
    ) -> Result<Option<ContextValue>, RemoteContextError> {
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
    ) -> Result<Option<Vec<(ContextKeyOwned, ContextValue)>>, RemoteContextError> {
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
    ) -> Result<StringTreeEntry, RemoteContextError> {
        Ok(self
            .tezos_readonly_api
            .pool
            .get()?
            .api
            .get_context_tree_by_prefix(context_hash, prefix, depth)?)
    }
}
