// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use shell_automaton::service::service_channel::{RequestSendError, ResponseTryRecvError};
pub use shell_automaton::service::storage_service::{
    StorageRequest, StorageResponse, StorageService,
};

#[derive(Debug, Clone)]
pub struct StorageServiceDummy {}

impl StorageServiceDummy {
    pub fn new() -> Self {
        Self {}
    }
}

impl StorageService for StorageServiceDummy {
    #[inline(always)]
    fn request_send(&mut self, _: StorageRequest) -> Result<(), RequestSendError<StorageRequest>> {
        Ok(())
    }

    #[inline(always)]
    fn response_try_recv(&mut self) -> Result<StorageResponse, ResponseTryRecvError> {
        Err(ResponseTryRecvError::Empty)
    }
}
