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
pub struct ProtocolEndpointConfiguration {
    #[get = "pub"]
    runtime_configuration: TezosRuntimeConfiguration,
    #[get_copy = "pub"]
    environment: TezosEnvironment,
    #[get = "pub"]
    data_dir: PathBuf,
    #[get = "pub"]
    executable_path: PathBuf
}

impl ProtocolEndpointConfiguration {
    pub fn new<P: AsRef<Path>>(runtime_configuration: TezosRuntimeConfiguration, environment: TezosEnvironment, data_dir: P, executable_path: P) -> Self {
        ProtocolEndpointConfiguration {
            runtime_configuration,
            environment,
            data_dir: data_dir.as_ref().into(),
            executable_path: executable_path.as_ref().into(),
        }
    }
}

pub struct IpcCmdServer(IpcServer<NodeMessage, ProtocolMessage>, ProtocolEndpointConfiguration);

impl IpcCmdServer {

    const IO_TIMEOUT: Duration = Duration::from_secs(10);

    pub fn new(configuration: ProtocolEndpointConfiguration,) -> Self {
        IpcCmdServer(IpcServer::bind_path(&temp_sock()).unwrap(), configuration)
    }

    pub fn accept(&mut self) -> Result<ProtocolController, IpcError> {
        let (rx, tx) = self.0.accept()?;
        // configure IO timeouts
        rx.set_read_timeout(Some(Self::IO_TIMEOUT))
            .and(tx.set_write_timeout(Some(Self::IO_TIMEOUT)))
            .map_err(|err| IpcError::SocketConfigurationError { reason: err })?;

        Ok(ProtocolController {
            io: Arc::new(Mutex::new(IpcIO { rx, tx })),
            configuration: &self.1
        })
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

/// Endpoint consists of a protocol runner and IPC communication (command and event channels).
pub struct ProtocolRunnerEndpoint {
    pub runner: ProtocolRunner,
    pub commands: IpcCmdServer,
    pub events: IpcEvtServer,
}

impl ProtocolRunnerEndpoint {
    pub fn new(configuration: ProtocolEndpointConfiguration) -> ProtocolRunnerEndpoint {
        let protocol_runner_path = configuration.executable_path.clone();
        let evt_server = IpcEvtServer::new();
        let cmd_server = IpcCmdServer::new(configuration);
        ProtocolRunnerEndpoint {
            runner: ProtocolRunner::new(&protocol_runner_path, cmd_server.0.client().path(), evt_server.0.client().path()),
            commands: cmd_server,
            events: evt_server,
        }
    }
}

struct IpcIO {
    rx: IpcReceiver<NodeMessage>,
    tx: IpcSender<ProtocolMessage>,
}

pub struct ProtocolController<'a> {
    io: Arc<Mutex<IpcIO>>,
    configuration: &'a ProtocolEndpointConfiguration,
}

impl<'a> ProtocolController<'a> {
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

    /// Gracefully shutdown protocol runner
    pub fn shutdown(&self) -> Result<(), ProtocolServiceError> {
        let mut io = self.io.lock().unwrap();
        io.tx.send(&ProtocolMessage::ShutdownCall)?;
        match io.rx.receive()? {
            NodeMessage::ShutdownResult => Ok(()),
            message => Err(ProtocolServiceError::UnexpectedMessage { message: message.into() }),
        }
    }

    /// Initialize protocol environment from default configuration.
    pub fn init_protocol(&self) ->Result<TezosStorageInitInfo, ProtocolServiceError> {
        self.change_runtime_configuration(self.configuration.runtime_configuration().clone())?;
        self.init_storage(self.configuration.data_dir().to_str().unwrap().to_string(), self.configuration.environment())
    }
}

impl Drop for ProtocolController<'_> {
    fn drop(&mut self) {
        // try to gracefully shutdown protocol runner
        let _ = self.shutdown();
    }
}

/// Holds information required to spawn protocol runner process.
pub struct ProtocolRunner {
    sock_cmd_path: PathBuf,
    sock_evt_path: PathBuf,
    executable_path: PathBuf,
}

impl ProtocolRunner {

    const PROCESS_WAIT_TIMEOUT: Duration = Duration::from_secs(4);

    pub fn new<P: AsRef<Path>>(executable_path: P, sock_cmd_path: &Path, sock_evt_path: &Path) -> Self {
        ProtocolRunner {
            sock_cmd_path: sock_cmd_path.to_path_buf(),
            sock_evt_path: sock_evt_path.to_path_buf(),
            executable_path: executable_path.as_ref().to_path_buf(),
        }
    }

    pub fn spawn(&self) -> Result<Child, ProtocolServiceError> {
        let process = Command::new(&self.executable_path)
            .arg("--sock-cmd")
            .arg(&self.sock_cmd_path)
            .arg("--sock-evt")
            .arg(&self.sock_evt_path)
            .spawn()
            .map_err(|err| ProtocolServiceError::SpawnError { reason: err })?;
        Ok(process)
    }

    pub fn terminate(mut process: Child) {
        match process.wait_timeout(Self::PROCESS_WAIT_TIMEOUT).unwrap() {
            Some(_) => (),
            None => {
                // child hasn't exited yet
                let _ = process.kill();
            }
        };
    }

    pub fn is_running(process: &mut Child) -> bool {
        match process.try_wait() {
            Ok(None) => true,
            _ => false,
        }
    }
}

