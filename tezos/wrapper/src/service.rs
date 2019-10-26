// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::convert::AsRef;
use std::path::Path;
use std::sync::Arc;

use failure::Fail;
use futures::lock::Mutex;
use serde::{Deserialize, Serialize};

use tezos_client::client::{TezosRuntimeConfiguration, TezosStorageInitInfo};
use tezos_client::environment::TezosEnvironment;
use tezos_encoding::hash::{BlockHash, ChainId};
use tezos_interop::ffi::*;
use tezos_messages::p2p::encoding::prelude::*;

use crate::api::*;
use crate::ipc::*;

#[derive(Serialize, Deserialize, Debug)]
enum ProtocolMessage {
    ApplyBlockCall(ApplyBlockParams),
    ChangeRuntimeConfigurationCall(TezosRuntimeConfiguration),
    InitStorageCall(InitStorageParams),
}

#[derive(Serialize, Deserialize, Debug)]
struct ApplyBlockParams {
    chain_id: ChainId,
    block_header_hash: BlockHash,
    block_header: BlockHeader,
    operations: Vec<Option<OperationsForBlocksMessage>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct InitStorageParams {
    storage_data_dir: String,
    tezos_environment: TezosEnvironment
}

#[derive(Serialize, Deserialize, Debug)]
enum NodeMessage {
    ApplyBlockResult(Result<ApplyBlockResult, ApplyBlockError>),
    ChangeRuntimeConfigurationResult(Result<(), OcamlRuntimeConfigurationError>),
    InitStorageResult(Result<TezosStorageInitInfo, OcamlStorageInitError>),
}

pub async fn process_protocol_messages<Proto: ProtocolApi, P: AsRef<Path>>(socket_path: P) -> Result<(), IpcError> {
    let ipc_client: IpcClient<ProtocolMessage, NodeMessage> = IpcClient::new(socket_path);
    let (mut rx, mut tx) = ipc_client.connect().await?;
    while let Ok(cmd) = rx.receive().await {
        match cmd {
            ProtocolMessage::ApplyBlockCall(params) => {
                let res = Proto::apply_block(&params.chain_id, &params.block_header_hash, &params.block_header, &params.operations);
                tx.send(&NodeMessage::ApplyBlockResult(res)).await?;
            }
            ProtocolMessage::ChangeRuntimeConfigurationCall(params) => {
                let res = Proto::change_runtime_configuration(params);
                tx.send(&NodeMessage::ChangeRuntimeConfigurationResult(res)).await?;
            }
            ProtocolMessage::InitStorageCall(params) => {
                let res = Proto::init_storage(params.storage_data_dir, params.tezos_environment);
                tx.send(&NodeMessage::InitStorageResult(res)).await?;
            }
        }
    }

    Ok(())
}

#[derive(Fail, Debug)]
pub enum ProtocolError {
    #[fail(display = "Apply block error: {}", reason)]
    ApplyBlockError {
        reason: ApplyBlockError
    },
    #[fail(display = "OCaml runtime configuration error: {}", reason)]
    OcamlRuntimeConfigurationError {
        reason: OcamlRuntimeConfigurationError
    },
    #[fail(display = "OCaml storage init error: {}", reason)]
    OcamlStorageInitError {
        reason: OcamlStorageInitError
    },
}

#[derive(Fail, Debug)]
pub enum ProtocolServiceError {
    #[fail(display = "IPC error: {}", reason)]
    IpcError {
        reason: IpcError
    },
    #[fail(display = "Protocol error: {}", reason)]
    ProtocolError {
        reason: ProtocolError
    },
}

impl From<IpcError> for ProtocolServiceError {
    fn from(error: IpcError) -> Self {
        ProtocolServiceError::IpcError { reason: error }
    }
}

impl From<ProtocolError> for ProtocolServiceError {
    fn from(error: ProtocolError) -> Self {
        ProtocolServiceError::ProtocolError { reason: error }
    }
}

#[allow(dead_code)]
pub struct IpcProtocolService {
    ipc_server: IpcServer<NodeMessage, ProtocolMessage>,
    inner: Arc<Mutex<ServiceInner>>,
}

struct ServiceInner {
    rx: IpcReceiver<NodeMessage>,
    tx: IpcSender<ProtocolMessage>,
}

impl IpcProtocolService {

    pub async fn bind_path<P: AsRef<Path>>(path: P) -> Self {
        let mut ipc_server = IpcServer::bind_path(path).unwrap();
        let (rx, tx)= ipc_server.accept().await.unwrap();
        IpcProtocolService {
            ipc_server,
            inner: Arc::new(Mutex::new(ServiceInner { rx, tx }))
        }
    }

    pub async fn apply_block(&self, chain_id: &Vec<u8>, block_header_hash: &Vec<u8>, block_header: &BlockHeader, operations: &Vec<Option<OperationsForBlocksMessage>>) -> Result<ApplyBlockResult, ProtocolServiceError> {
        let mut inner = self.inner.lock().await;
        let ServiceInner {rx, tx} = &mut *inner;
        tx.send(&ProtocolMessage::ApplyBlockCall(ApplyBlockParams {
            chain_id: chain_id.clone(),
            block_header_hash: block_header_hash.clone(),
            block_header: block_header.clone(),
            operations: operations.clone(),
        })).await?;
        match rx.receive().await? {
            NodeMessage::ApplyBlockResult(result) => result.map_err(|err| ProtocolError::ApplyBlockError { reason: err }.into()),
            _ => panic!("unexpected message received")
        }
    }

    pub async fn change_runtime_configuration(&self, settings: TezosRuntimeConfiguration) -> Result<(), ProtocolServiceError> {
        let mut inner = self.inner.lock().await;
        let ServiceInner {rx, tx} = &mut *inner;
        tx.send(&ProtocolMessage::ChangeRuntimeConfigurationCall(settings)).await?;
        match rx.receive().await? {
            NodeMessage::ChangeRuntimeConfigurationResult(result) => result.map_err(|err| ProtocolError::OcamlRuntimeConfigurationError { reason: err }.into()),
            _ => panic!("unexpected message received")
        }
    }

    pub async fn init_storage(&self, storage_data_dir: String, tezos_environment: TezosEnvironment) -> Result<TezosStorageInitInfo, ProtocolServiceError> {
        let mut inner = self.inner.lock().await;
        let ServiceInner {rx, tx} = &mut *inner;
        tx.send(&ProtocolMessage::InitStorageCall(InitStorageParams {
            storage_data_dir,
            tezos_environment
        })).await?;
        match rx.receive().await? {
            NodeMessage::InitStorageResult(result) => result.map_err(|err| ProtocolError::OcamlStorageInitError { reason: err }.into()),
            _ => panic!("unexpected message received")
        }
    }
}

