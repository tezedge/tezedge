// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use thiserror::Error;
use tokio::sync::mpsc;

use tezos_api::ffi::{BeginConstructionRequest, ValidateOperationRequest};
use tezos_api::ffi::{PrevalidatorWrapper, ValidateOperationResponse};
use tezos_protocol_ipc_client::{ProtocolRunnerApi, ProtocolServiceError};

pub trait ProtocolService {
    fn try_recv(&mut self) -> Result<ProtocolResponse, ProtocolError>;

    fn begin_construction_for_prevalidation(&mut self, request: BeginConstructionRequest);
    fn validate_operation_for_prevalidation(&mut self, request: ValidateOperationRequest);
    fn begin_construction_for_mempool(&mut self, request: BeginConstructionRequest);
    fn validate_operation_for_mempool(&mut self, request: ValidateOperationRequest);
}

// TODO: use tezos_protocol_ipc_messages::NodeMessage
#[derive(Debug, Clone)]
pub enum ProtocolResponse {
    PrevalidatorReady(PrevalidatorWrapper),
    PrevalidatorForMempoolReady(PrevalidatorWrapper),
    OperationValidated(ValidateOperationResponse),
}

// TODO: use tezos_protocol_ipc_messages::ProtocolMessage
pub enum ProtocolRequest {
    BeginConstructionForPrevalidationCall(BeginConstructionRequest),
    ValidateOperationForPrevalidationCall(ValidateOperationRequest),
    BeginConstructionForMempoolCall(BeginConstructionRequest),
    ValidateOperationForMempoolCall(ValidateOperationRequest),
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("receiving on an empty channel")]
    Empty,
    #[error("receiving on a closed channel")]
    Disconnected,
    #[error("{_0}")]
    Internal(ProtocolServiceError),
}

pub struct ProtocolServiceDefault {
    response_rx: mpsc::Receiver<Result<ProtocolResponse, ProtocolError>>,
    request_tx: mpsc::Sender<ProtocolRequest>,
    task: tokio::task::JoinHandle<()>,
}

impl ProtocolServiceDefault {
    pub fn new(mio_waker: Arc<mio::Waker>, api: Arc<ProtocolRunnerApi>) -> Self {
        let (response_tx, response_rx) = mpsc::channel(1024);
        let (request_tx, request_rx) = mpsc::channel(1);
        ProtocolServiceDefault {
            response_rx,
            request_tx,
            task: {
                let _guard = api.tokio_runtime.enter();
                tokio::spawn(async move {
                    let mut request_rx = request_rx;
                    let mut connection = api.readable_connection().await.unwrap();
                    while let Some(request) = request_rx.recv().await {
                        let response = match request {
                            ProtocolRequest::BeginConstructionForPrevalidationCall(request) => {
                                connection
                                    .begin_construction_for_prevalidation(request)
                                    .await
                                    .map(ProtocolResponse::PrevalidatorReady)
                            }
                            ProtocolRequest::BeginConstructionForMempoolCall(request) => connection
                                .begin_construction_for_mempool(request)
                                .await
                                .map(ProtocolResponse::PrevalidatorForMempoolReady),
                            ProtocolRequest::ValidateOperationForPrevalidationCall(request) => {
                                connection
                                    .validate_operation_for_prevalidation(request)
                                    .await
                                    .map(ProtocolResponse::OperationValidated)
                            }
                            ProtocolRequest::ValidateOperationForMempoolCall(request) => connection
                                .validate_operation_for_mempool(request)
                                .await
                                .map(ProtocolResponse::OperationValidated),
                        };
                        let _ = response_tx.send(response.map_err(ProtocolError::Internal)).await;
                        let _ = mio_waker.wake();
                    }
                })
            },
        }
    }

    // TODO: use graceful shutdown
    pub async fn join_gracefully(self) {
        match self.task.await {
            Ok(()) => (),
            Err(error) => {
                if error.is_cancelled() {
                    // log the task was canceled
                } else if error.is_panic() {
                    // log the task panic
                    let _ = error.into_panic();
                } else {
                    // log error
                    let _ = error;
                }
            }
        }
    }
}

impl ProtocolService for ProtocolServiceDefault {
    fn try_recv(&mut self) -> Result<ProtocolResponse, ProtocolError> {
        self.response_rx
            .try_recv()
            .map_err(|e| match e {
                mpsc::error::TryRecvError::Disconnected => ProtocolError::Disconnected,
                mpsc::error::TryRecvError::Empty => ProtocolError::Empty,
            })
            .and_then(|v| v)
    }

    fn begin_construction_for_prevalidation(&mut self, request: BeginConstructionRequest) {
        let request = ProtocolRequest::BeginConstructionForPrevalidationCall(request);
        let _ = self.request_tx.blocking_send(request);
    }

    fn validate_operation_for_prevalidation(&mut self, request: ValidateOperationRequest) {
        let request = ProtocolRequest::ValidateOperationForPrevalidationCall(request);
        let _ = self.request_tx.blocking_send(request);
    }

    fn begin_construction_for_mempool(&mut self, request: BeginConstructionRequest) {
        let request = ProtocolRequest::BeginConstructionForMempoolCall(request);
        let _ = self.request_tx.blocking_send(request);
    }

    fn validate_operation_for_mempool(&mut self, request: ValidateOperationRequest) {
        let request = ProtocolRequest::ValidateOperationForMempoolCall(request);
        let _ = self.request_tx.blocking_send(request);
    }
}
