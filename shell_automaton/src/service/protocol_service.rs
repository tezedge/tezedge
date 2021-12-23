// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::HashMap, future::Future, sync::Arc};

use thiserror::Error;
use tokio::{sync::mpsc, sync::Mutex};

use tezos_api::ffi::{BeginConstructionRequest, ValidateOperationRequest};
use tezos_api::ffi::{InitProtocolContextResult, PrevalidatorWrapper, ValidateOperationResponse};
use tezos_protocol_ipc_client::{
    ProtocolRunnerApi, ProtocolRunnerConnection, ProtocolServiceError,
};

pub trait ProtocolService {
    fn try_recv(&mut self) -> Result<ProtocolResponse, ProtocolError>;

    fn init_protocol_for_read(&mut self);
    fn begin_construction_for_prevalidation(&mut self, request: BeginConstructionRequest);
    fn validate_operation_for_prevalidation(&mut self, request: ValidateOperationRequest);
    fn begin_construction_for_mempool(&mut self, request: BeginConstructionRequest);
    fn validate_operation_for_mempool(&mut self, request: ValidateOperationRequest);
}

#[derive(Debug, Clone)]
pub enum ProtocolResponse {
    InitProtocolDone(InitProtocolContextResult),
    PrevalidatorReady(PrevalidatorWrapper),
    PrevalidatorForMempoolReady(PrevalidatorWrapper),
    OperationValidated(ValidateOperationResponse),
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
    api: Arc<ProtocolRunnerApi>,
    connection: Arc<Mutex<Option<ProtocolRunnerConnection>>>,
    responses: mpsc::Receiver<(Result<ProtocolResponse, ProtocolError>, usize)>,
    sender: mpsc::Sender<(Result<ProtocolResponse, ProtocolError>, usize)>,
    mio_waker: Arc<mio::Waker>,
    counter: usize,
    tasks: HashMap<usize, tokio::task::JoinHandle<()>>,
}

impl ProtocolServiceDefault {
    pub fn new(mio_waker: Arc<mio::Waker>, api: Arc<ProtocolRunnerApi>) -> Self {
        let (tx, rx) = mpsc::channel(1024);
        ProtocolServiceDefault {
            api,
            connection: Arc::new(Mutex::new(None)),
            responses: rx,
            sender: tx,
            mio_waker,
            counter: 0,
            tasks: HashMap::new(),
        }
    }

    // `op` takes mutex with the connection, the connection cannot be `None`, it is safe to unwrap
    fn spawn<F>(
        &mut self,
        op: impl FnOnce(Arc<Mutex<Option<ProtocolRunnerConnection>>>) -> F + Send + 'static,
    ) where
        F: Future<Output = Result<ProtocolResponse, ProtocolServiceError>> + Send,
    {
        let api = self.api.clone();
        let sender = self.sender.clone();
        let waker = self.mio_waker.clone();
        let id = self.counter;
        self.counter = self.counter.wrapping_add(1);
        let _guard = self.api.tokio_runtime.enter();
        let cn = Arc::clone(&self.connection);
        self.tasks.insert(
            id,
            tokio::spawn(async move {
                let result = match Self::task(&api, cn, op).await {
                    Ok(response) => Ok(response),
                    Err(err) => Err(ProtocolError::Internal(err)),
                };
                let _ = sender.send((result, id)).await;
                let _ = waker.wake(); // cannot we handle it
            }),
        );
    }

    async fn task<F>(
        api: &ProtocolRunnerApi,
        cn: Arc<Mutex<Option<ProtocolRunnerConnection>>>,
        op: impl FnOnce(Arc<Mutex<Option<ProtocolRunnerConnection>>>) -> F + Send + 'static,
    ) -> Result<ProtocolResponse, ProtocolServiceError>
    where
        F: Future<Output = Result<ProtocolResponse, ProtocolServiceError>> + Send,
    {
        let mut cn_lock = cn.lock().await;
        if cn_lock.is_none() {
            let connection = api.readable_connection().await?;
            *cn_lock = Some(connection);
        }
        drop(cn_lock);
        // the `cn` inside `op` cannot be None, because it is checked few lines above
        op(cn).await
    }
}

impl ProtocolService for ProtocolServiceDefault {
    fn try_recv(&mut self) -> Result<ProtocolResponse, ProtocolError> {
        self.responses
            .try_recv()
            .map_err(|e| match e {
                mpsc::error::TryRecvError::Disconnected => ProtocolError::Disconnected,
                mpsc::error::TryRecvError::Empty => ProtocolError::Empty,
            })
            .and_then(|(response, id)| {
                let _ = self.tasks.remove(&id); // it is already done
                response
            })
    }

    fn init_protocol_for_read(&mut self) {
        self.spawn(|connection| async move {
            connection
                .lock()
                .await
                .as_mut()
                // the `cn` here cannot be None, it is a contract of `Self::spawn` method
                .unwrap()
                .init_protocol_for_read()
                .await
                .map(ProtocolResponse::InitProtocolDone)
        })
    }

    fn begin_construction_for_prevalidation(&mut self, request: BeginConstructionRequest) {
        self.spawn(|connection| async move {
            connection
                .lock()
                .await
                .as_mut()
                // the `cn` here cannot be None, it is a contract of `Self::spawn` method
                .unwrap()
                .begin_construction_for_prevalidation(request)
                .await
                .map(ProtocolResponse::PrevalidatorReady)
        })
    }

    fn validate_operation_for_prevalidation(&mut self, request: ValidateOperationRequest) {
        // TODO(vlad): remove it
        // let hash = tezos_messages::p2p::binary_message::MessageHash::message_typed_hash::<crypto::hash::OperationHash>(&request.operation).unwrap();
        self.spawn(|connection| async move {
            connection
                .lock()
                .await
                .as_mut()
                // the `cn` here cannot be None, it is a contract of `Self::spawn` method
                .unwrap()
                .validate_operation_for_prevalidation(request)
                .await
                .map(ProtocolResponse::OperationValidated)
        })
    }

    fn begin_construction_for_mempool(&mut self, request: BeginConstructionRequest) {
        self.spawn(|connection| async move {
            connection
                .lock()
                .await
                .as_mut()
                // the `cn` here cannot be None, it is a contract of `Self::spawn` method
                .unwrap()
                .begin_construction_for_mempool(request)
                .await
                .map(ProtocolResponse::PrevalidatorForMempoolReady)
        })
    }

    fn validate_operation_for_mempool(&mut self, request: ValidateOperationRequest) {
        self.spawn(|connection| async move {
            connection
                .lock()
                .await
                .as_mut()
                // the `cn` here cannot be None, it is a contract of `Self::spawn` method
                .unwrap()
                .validate_operation_for_mempool(request)
                .await
                .map(ProtocolResponse::OperationValidated)
        })
    }
}
