// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::VecDeque;

use storage::StorageInitInfo;
use tezos_api::ffi::CommitGenesisResult;

use shell_automaton::service::service_channel::{RequestSendError, ResponseTryRecvError};
use shell_automaton::service::storage_service::StorageError;
pub use shell_automaton::service::storage_service::{
    StorageRequest, StorageResponse, StorageService,
};

#[derive(Debug, Clone)]
pub struct StorageServiceDummy {
    pub requests: VecDeque<StorageRequest>,
    pub responses: VecDeque<StorageResponse>,
}

impl StorageServiceDummy {
    pub fn new() -> Self {
        Self {
            requests: Default::default(),
            responses: Default::default(),
        }
    }
}

impl StorageService for StorageServiceDummy {
    #[inline(always)]
    fn request_send(
        &mut self,
        req: StorageRequest,
    ) -> Result<(), RequestSendError<StorageRequest>> {
        self.requests.push_back(req);
        Ok(())
    }

    #[inline(always)]
    fn response_try_recv(&mut self) -> Result<StorageResponse, ResponseTryRecvError> {
        self.responses
            .pop_front()
            .map(Ok)
            .unwrap_or(Err(ResponseTryRecvError::Empty))
    }

    fn blocks_genesis_commit_result_put(
        &mut self,
        _: &StorageInitInfo,
        _: CommitGenesisResult,
    ) -> Result<(), StorageError> {
        Ok(())
    }
}
