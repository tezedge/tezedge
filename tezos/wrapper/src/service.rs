// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::AsRef;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

use failure::Fail;
use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};
use strum_macros::IntoStaticStr;
use wait_timeout::ChildExt;

use ipc::*;
use tezos_api::client::TezosStorageInitInfo;
use tezos_api::environment::TezosEnvironment;
use tezos_api::ffi::*;
use tezos_encoding::hash::{BlockHash, ChainId};
use tezos_messages::p2p::encoding::prelude::*;

use crate::protocol::*;
use std::sync::{Mutex, Arc};

#[derive(Serialize, Deserialize, Debug, IntoStaticStr)]
enum ProtocolMessage {
    ApplyBlockCall(ApplyBlockParams),
    ChangeRuntimeConfigurationCall(TezosRuntimeConfiguration),
    InitStorageCall(InitStorageParams),
    ShutdownCall,
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

#[derive(Serialize, Deserialize, Debug, IntoStaticStr)]
enum NodeMessage {
    ApplyBlockResult(Result<ApplyBlockResult, ApplyBlockError>),
    ChangeRuntimeConfigurationResult(Result<(), TezosRuntimeConfigurationError>),
    InitStorageResult(Result<TezosStorageInitInfo, TezosStorageInitError>),
    ShutdownResult
}

pub fn process_protocol_messages<Proto: ProtocolApi, P: AsRef<Path>>(socket_path: P) -> Result<(), IpcError> {
    let ipc_client: IpcClient<ProtocolMessage, NodeMessage> = IpcClient::new(socket_path);
    let (mut rx, mut tx) = ipc_client.connect()?;
    while let Ok(cmd) = rx.receive() {
        match cmd {
            ProtocolMessage::ApplyBlockCall(params) => {
                let res = Proto::apply_block(&params.chain_id, &params.block_header_hash, &params.block_header, &params.operations);
                tx.send(&NodeMessage::ApplyBlockResult(res))?;
            }
            ProtocolMessage::ChangeRuntimeConfigurationCall(params) => {
                let res = Proto::change_runtime_configuration(params);
                tx.send(&NodeMessage::ChangeRuntimeConfigurationResult(res))?;
            }
            ProtocolMessage::InitStorageCall(params) => {
                let res = Proto::init_storage(params.storage_data_dir, params.tezos_environment);
                tx.send(&NodeMessage::InitStorageResult(res))?;
            }
            ProtocolMessage::ShutdownCall => {
                tx.send(&NodeMessage::ShutdownResult)?;
                break;
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
    TezosRuntimeConfigurationError {
        reason: TezosRuntimeConfigurationError
    },
    #[fail(display = "OCaml storage init error: {}", reason)]
    OcamlStorageInitError {
        reason: TezosStorageInitError
    },
}

#[derive(Fail, Debug)]
pub enum ProtocolServiceError {
    #[fail(display = "IPC error: {}", reason)]
    IpcError {
        reason: IpcError,
    },
    #[fail(display = "Protocol error: {}", reason)]
    ProtocolError {
        reason: ProtocolError,
    },
    #[fail(display = "Received unexpected message: {}", message)]
    UnexpectedMessage {
        message: &'static str,
    },
    #[fail(display = "Failed to spawn tezos protocol wrapper sub-process: {}", reason)]
    SpawnError {
        reason: io::Error,
    },
}

impl slog::Value for ProtocolServiceError {
    fn serialize(&self, _record: &slog::Record, key: slog::Key, serializer: &mut dyn slog::Serializer) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
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

#[derive(Clone, Getters, CopyGetters)]
pub struct ProtocolServiceConfiguration {
    #[get = "pub"]
    runtime_configuration: TezosRuntimeConfiguration,
    #[get_copy = "pub"]
    environment: TezosEnvironment,
    #[get = "pub"]
    data_dir: PathBuf,
    #[get = "pub"]
    executable_path: PathBuf
}

impl ProtocolServiceConfiguration {
    pub fn new<P: AsRef<Path>>(runtime_configuration: TezosRuntimeConfiguration, environment: TezosEnvironment, data_dir: P, executable_path: P) -> Self {
        ProtocolServiceConfiguration {
            runtime_configuration,
            environment,
            data_dir: data_dir.as_ref().into(),
            executable_path: executable_path.as_ref().into(),
        }
    }
}

pub struct ProtocolService {
    configuration: ProtocolServiceConfiguration,
    ipc_server: IpcServer<NodeMessage, ProtocolMessage>,
}

impl ProtocolService {

    pub fn bind(configuration: ProtocolServiceConfiguration) -> Self {
        let ipc_server = IpcServer::bind_path(&temp_sock()).unwrap();
        ProtocolService { configuration, ipc_server }
    }

    pub fn spawn_protocol_wrapper(&mut self) -> Result<ProtocolWrapperIpc, ProtocolServiceError> {
        let client = self.ipc_server.client();
        let sock_path = client.path();
        let process = Command::new(&self.configuration.executable_path)
            .arg("--sock")
            .arg(sock_path)
            .spawn()
            .map_err(|err| ProtocolServiceError::SpawnError { reason: err })?;
        let (rx, tx) = self.ipc_server.accept()?;
        Ok(ProtocolWrapperIpc { process, io: Arc::new(Mutex::new(ProtocolWrapperIO { rx, tx })) })
    }

    pub fn configuration(&self) -> &ProtocolServiceConfiguration {
        &self.configuration
    }
}

struct ProtocolWrapperIO {
    rx: IpcReceiver<NodeMessage>,
    tx: IpcSender<ProtocolMessage>,
}

impl Drop for ProtocolWrapperIO {
    fn drop(&mut self) {
        let _ = self.tx.shutdown();
        let _ = self.rx.shutdown();
    }
}

pub struct ProtocolWrapperIpc {
    process: Child,
    io: Arc<Mutex<ProtocolWrapperIO>>,
}

impl Drop for ProtocolWrapperIpc {
    fn drop(&mut self) {
        // send shutdown message to gracefully shutdown child process
        let _ = self.shutdown();
        let wait_timeout = Duration::from_secs(2);
        match self.process.wait_timeout(wait_timeout).unwrap() {
            Some(_) => (),
            None => {
                // child hasn't exited yet
                let _ = self.process.kill();
            }
        };
    }
}

impl ProtocolWrapperIpc {
    pub fn apply_block(&self, chain_id: &Vec<u8>, block_header_hash: &Vec<u8>, block_header: &BlockHeader, operations: &Vec<Option<OperationsForBlocksMessage>>) -> Result<ApplyBlockResult, ProtocolServiceError> {
        let mut io = self.io.lock().unwrap();
        io.tx.send(&ProtocolMessage::ApplyBlockCall(ApplyBlockParams {
            chain_id: chain_id.clone(),
            block_header_hash: block_header_hash.clone(),
            block_header: block_header.clone(),
            operations: operations.clone(),
        }))?;
        match io.rx.receive()? {
            NodeMessage::ApplyBlockResult(result) => result.map_err(|err| ProtocolError::ApplyBlockError { reason: err }.into()),
            message => Err(ProtocolServiceError::UnexpectedMessage { message: message.into() })
        }
    }

    pub fn change_runtime_configuration(&self, settings: TezosRuntimeConfiguration) -> Result<(), ProtocolServiceError> {
        let mut io = self.io.lock().unwrap();
        io.tx.send(&ProtocolMessage::ChangeRuntimeConfigurationCall(settings))?;
        match io.rx.receive()? {
            NodeMessage::ChangeRuntimeConfigurationResult(result) => result.map_err(|err| ProtocolError::TezosRuntimeConfigurationError { reason: err }.into()),
            message => Err(ProtocolServiceError::UnexpectedMessage { message: message.into() })
        }
    }

    pub fn init_storage(&self, storage_data_dir: String, tezos_environment: TezosEnvironment) -> Result<TezosStorageInitInfo, ProtocolServiceError> {
        let mut io = self.io.lock().unwrap();
        io.tx.send(&ProtocolMessage::InitStorageCall(InitStorageParams {
            storage_data_dir,
            tezos_environment
        }))?;
        match io.rx.receive()? {
            NodeMessage::InitStorageResult(result) => result.map_err(|err| ProtocolError::OcamlStorageInitError { reason: err }.into()),
            message => Err(ProtocolServiceError::UnexpectedMessage { message: message.into() })
        }
    }

    pub fn shutdown(&self) -> Result<(), ProtocolServiceError> {
        let mut io = self.io.lock().unwrap();
        io.tx.send(&ProtocolMessage::ShutdownCall)?;
        match io.rx.receive()? {
            NodeMessage::ShutdownResult => Ok(()),
            message => Err(ProtocolServiceError::UnexpectedMessage { message: message.into() }),
        }
    }
}
