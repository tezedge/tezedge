// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::AsRef;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
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
use tezos_context::channel::{context_receive, context_send, ContextAction};
use tezos_encoding::hash::{BlockHash, ChainId};
use tezos_messages::p2p::encoding::prelude::*;

use crate::protocol::*;

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

#[derive(Serialize, Deserialize, Debug)]
struct NoopMessage;

pub fn process_protocol_events<P: AsRef<Path>>(socket_path: P) -> Result<(), IpcError> {
    let ipc_client: IpcClient<NoopMessage, ContextAction> = IpcClient::new(socket_path);
    let (_, mut tx) = ipc_client.connect()?;
    while let Ok(action) = context_receive() {
        tx.send(&action)?;
        if let ContextAction::Shutdown = action {
            break;
        }
    }

    Ok(())
}

pub fn process_protocol_commands<Proto: ProtocolApi, P: AsRef<Path>>(socket_path: P) -> Result<(), IpcError> {
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
                context_send(ContextAction::Shutdown).expect("Failed to send shutdown command to context channel");
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
    cmd_server: IpcCmdServer,
    sock_evt_path: PathBuf,
    configuration: ProtocolServiceConfiguration,
}

pub struct IpcCmdServer(IpcServer<NodeMessage, ProtocolMessage>);

impl IpcCmdServer {
    pub fn new() -> Self {
        IpcCmdServer(IpcServer::bind_path(&temp_sock()).unwrap())
    }
}

pub struct IpcEvtServer(IpcServer<ContextAction, NoopMessage>);

impl IpcEvtServer {
    pub fn new() -> Self {
        IpcEvtServer(IpcServer::bind_path(&temp_sock()).unwrap())
    }

    pub fn accept(&mut self) -> Result<IpcReceiver<ContextAction>, IpcError> {
        let (rx, _) = self.0.accept()?;
        Ok(rx)
    }
}

/// Endpoint consists of a `service` which is used to send commands to protocol runner
/// and an `events` channel which is used to accept various events generated by a protocol runner.
pub struct ProtocolServiceEndpoint {
    pub service: ProtocolService,
    pub events: IpcEvtServer,
}

impl ProtocolService {

    const IO_TIMEOUT: Duration = Duration::from_secs(5);

    pub fn create_endpoint(configuration: ProtocolServiceConfiguration) -> ProtocolServiceEndpoint {
        let evt_server = IpcEvtServer::new();
        ProtocolServiceEndpoint {
            service: ProtocolService {
                cmd_server: IpcCmdServer::new(),
                sock_evt_path: evt_server.0.client().path().into(),
                configuration,
            },
            events: evt_server,
        }
    }

    pub fn spawn_protocol_wrapper(&mut self) -> Result<ProtocolWrapperIpc, ProtocolServiceError> {
        let client_cmd = self.cmd_server.0.client();
        let sock_cmd_path = client_cmd.path();
        let process = Command::new(&self.configuration.executable_path)
            .arg("--sock-cmd")
            .arg(sock_cmd_path)
            .arg("--sock-evt")
            .arg(&self.sock_evt_path)
            .spawn()
            .map_err(|err| ProtocolServiceError::SpawnError { reason: err })?;
        let (rx, tx) = self.cmd_server.0.accept()?;
        rx.set_read_timeout(Some(Self::IO_TIMEOUT))
            .and(tx.set_write_timeout(Some(Self::IO_TIMEOUT)))
            .map_err(|err| ProtocolServiceError::IpcError { reason: IpcError::SocketConfigurationError { reason: err } })?;
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
