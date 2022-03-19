// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::HashMap, future::Future, sync::Arc};

use tokio::{sync::mpsc, sync::Mutex};

use tezos_api::ffi::{BeginConstructionRequest, ValidateOperationRequest};
use tezos_protocol_ipc_client::{
    ProtocolRunnerApi, ProtocolRunnerConnection, ProtocolServiceError,
};

use crate::protocol::ProtocolAction;

// TODO(vlad): proper error
pub trait PrevalidatorService {
    fn try_recv(&mut self) -> Result<ProtocolAction, ()>;

    fn begin_construction_for_prevalidation(&mut self, request: BeginConstructionRequest);
    fn validate_operation_for_prevalidation(&mut self, request: ValidateOperationRequest);
    fn begin_construction_for_mempool(&mut self, request: BeginConstructionRequest);
    fn validate_operation_for_mempool(&mut self, request: ValidateOperationRequest);
}

pub struct PrevalidatorServiceDefault {
    api: Arc<ProtocolRunnerApi>,
    connection: Arc<Mutex<Option<ProtocolRunnerConnection>>>,
    responses: mpsc::Receiver<(ProtocolAction, usize)>,
    sender: mpsc::Sender<(ProtocolAction, usize)>,
    mio_waker: Arc<mio::Waker>,
    counter: usize,
    tasks: HashMap<usize, tokio::task::JoinHandle<()>>,
}

impl PrevalidatorServiceDefault {
    pub fn new(mio_waker: Arc<mio::Waker>, api: Arc<ProtocolRunnerApi>) -> Self {
        let (tx, rx) = mpsc::channel(1024);
        PrevalidatorServiceDefault {
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
        F: Future<Output = Result<ProtocolAction, ProtocolServiceError>> + Send,
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
                let action = match Self::task(&api, cn, op).await {
                    Ok(response) => response,
                    Err(err) => ProtocolAction::Error(err),
                };
                let _ = sender.send((action, id)).await;
                let _ = waker.wake(); // cannot we handle it
            }),
        );
    }

    async fn task<F>(
        api: &ProtocolRunnerApi,
        cn: Arc<Mutex<Option<ProtocolRunnerConnection>>>,
        op: impl FnOnce(Arc<Mutex<Option<ProtocolRunnerConnection>>>) -> F + Send + 'static,
    ) -> Result<ProtocolAction, String>
    where
        F: Future<Output = Result<ProtocolAction, ProtocolServiceError>> + Send,
    {
        let mut cn_lock = cn.lock().await;
        if cn_lock.is_none() {
            let connection = api
                .readable_connection()
                .await
                .map_err(|err| err.to_string())?;
            *cn_lock = Some(connection);
        }
        drop(cn_lock);
        // the `cn` inside `op` cannot be None, because it is checked few lines above
        op(cn).await.map_err(|err| err.to_string())
    }
}

impl PrevalidatorService for PrevalidatorServiceDefault {
    fn try_recv(&mut self) -> Result<ProtocolAction, ()> {
        self.responses
            .try_recv()
            .map(|(response, id)| {
                let handle = self.tasks.remove(&id);
                let _ = handle; // it is already done
                response
            })
            .map_err(|_| ())
    }

    fn begin_construction_for_prevalidation(&mut self, request: BeginConstructionRequest) {
        // Abort all prevalidation tasks as they are no longer relevant.
        self.tasks.drain().for_each(|(_, task)| task.abort());
        while self.responses.try_recv().is_ok() {}
        self.connection = Arc::new(Mutex::new(None));

        self.spawn(|connection| async move {
            connection
                .lock()
                .await
                .as_mut()
                // the `cn` here cannot be None, it is a contract of `Self::spawn` method
                .unwrap()
                .begin_construction_for_prevalidation(request)
                .await
                .map(ProtocolAction::PrevalidatorReady)
        })
    }

    fn validate_operation_for_prevalidation(&mut self, request: ValidateOperationRequest) {
        self.spawn(|connection| async move {
            connection
                .lock()
                .await
                .as_mut()
                // the `cn` here cannot be None, it is a contract of `Self::spawn` method
                .unwrap()
                .validate_operation_for_prevalidation(request)
                .await
                .map(ProtocolAction::OperationValidated)
        })
    }

    fn begin_construction_for_mempool(&mut self, request: BeginConstructionRequest) {
        // Abort all prevalidation tasks as they are no longer relevant.
        // self.tasks.drain().for_each(|(_, task)| task.abort());

        self.spawn(|connection| async move {
            connection
                .lock()
                .await
                .as_mut()
                // the `cn` here cannot be None, it is a contract of `Self::spawn` method
                .unwrap()
                .begin_construction_for_mempool(request)
                .await
                .map(ProtocolAction::PrevalidatorForMempoolReady)
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
                .map(ProtocolAction::OperationValidated)
        })
    }
}
