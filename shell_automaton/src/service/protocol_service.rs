// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{sync::Arc, future::Future};

use tokio::sync::mpsc;
use slab::Slab;

use tezos_protocol_ipc_client::{ProtocolServiceError, ProtocolRunnerApi, ProtocolRunnerConnection};
use tezos_api::ffi::{BeginConstructionRequest, ValidateOperationRequest};

use crate::protocol::ProtocolAction;

// TODO(vlad): proper error
pub trait ProtocolService {
    fn try_recv(&mut self) -> Result<ProtocolAction, ()>;

    fn init_protocol_for_read(&mut self);
    fn begin_construction_for_prevalidation(&mut self, request: BeginConstructionRequest);
    fn begin_construction_for_mempool(&mut self, request: BeginConstructionRequest);
    fn validate_operation_for_mempool(&mut self, request: ValidateOperationRequest);
}

pub struct ProtocolServiceDefault {
    api: Arc<ProtocolRunnerApi>,
    responses: mpsc::Receiver<(ProtocolAction, usize)>,
    sender: mpsc::Sender<(ProtocolAction, usize)>,
    mio_waker: Arc<mio::Waker>,
    tasks: Slab<tokio::task::JoinHandle<()>>,
}

impl ProtocolServiceDefault {
    pub fn new(mio_waker: Arc<mio::Waker>, api: Arc<ProtocolRunnerApi>) -> Self {
        let (tx, rx) = mpsc::channel(8);
        ProtocolServiceDefault {
            api,
            responses: rx,
            sender: tx,
            mio_waker,
            tasks: Slab::new(),
        }
    }

    fn spawn<F>(&mut self, op: impl FnOnce(ProtocolRunnerConnection) -> F + Send + 'static)
    where
        F: Future<Output = Result<ProtocolAction, ProtocolServiceError>> + Send + 'static,
    {
        let api = self.api.clone();
        let sender = self.sender.clone();
        let waker = self.mio_waker.clone();
        let entry = self.tasks.vacant_entry();
        let id = entry.key();
        let _guard = self.api.tokio_runtime.enter();
        entry.insert(tokio::spawn(async move {
            let action = match api.readable_connection().await {
                Ok(connection) => match op(connection).await {
                    Ok(response) => response,
                    Err(err) => ProtocolAction::Error(err.to_string()),
                },
                Err(err) => ProtocolAction::Error(err.to_string()),
            };
            let _ = sender.send((action, id)).await;
            let _ = waker.wake(); // TODO(vlad): how can we handle it?
        }));
    }
}

impl ProtocolService for ProtocolServiceDefault {
    fn try_recv(&mut self) -> Result<ProtocolAction, ()> {
        self.responses.try_recv()
            .map(|(response, id)| {
                let handle = self.tasks.remove(id);
                let _ = handle; // it is already done
                response
            })
            .map_err(|_| ())
    }

    fn init_protocol_for_read(&mut self) {
        self.spawn(|mut connection| async move {
            connection.init_protocol_for_read().await.map(ProtocolAction::InitProtocolDone)
        })
    }

    fn begin_construction_for_prevalidation(&mut self, request: BeginConstructionRequest) {
        self.spawn(|mut connection| async move {
            connection.begin_construction_for_prevalidation(request)
                .await
                .map(ProtocolAction::PrevalidatorReady)
        })
    }

    fn begin_construction_for_mempool(&mut self, request: BeginConstructionRequest) {
        self.spawn(|mut connection| async move {
            connection.begin_construction_for_mempool(request)
                .await
                .map(ProtocolAction::PrevalidatorForMempoolReady)
        })
    }

    fn validate_operation_for_mempool(&mut self, request: ValidateOperationRequest) {
        self.spawn(|mut connection| async move {
            connection.validate_operation_for_mempool(request)
                .await
                .map(ProtocolAction::OperationValidated)
        })
    }
}
