// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]
#![cfg_attr(feature = "fuzzing", feature(no_coverage))]

//! Provides Async IPC communication.
//!
//! The IPC is implemented as unix domain sockets. Functionality is similar to how network sockets work.
//!

use std::env;
use std::fs;
use std::iter;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// IPC communication errors
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Error, Serialize, Deserialize, Debug, Clone)]
pub enum IpcError {
    #[error("Receive message length error: {reason}")]
    ReceiveMessageLengthError { reason: String },
    #[error("Receive message error: {reason}")]
    ReceiveMessageError { reason: String },
    #[error("Send error: {reason}")]
    SendError { reason: String },
    #[error("Accept connection timed out, timout: {timeout:?}")]
    AcceptTimeout { timeout: Duration },
    #[error("Receive message timed out")]
    ReceiveMessageTimeout,
    #[error("Connection error: {reason}")]
    ConnectionError { reason: String },
    #[error("Serialization error: {reason}")]
    SerializationError { reason: String },
    #[error("Deserialization error: {reason}")]
    DeserializationError { reason: String },
    #[error("IPC error: {reason}")]
    OtherError { reason: String },
}

/// Represents sending end of the IPC channel.
pub struct IpcSender<S>(OwnedWriteHalf, PhantomData<S>);

impl<S> IpcSender<S> {
    /// Close IPC channel and release associated resources.
    ///
    /// This closes only the sending part of the IPC channel.
    async fn shutdown(&mut self) -> io::Result<()> {
        self.0.shutdown().await
    }
}

impl<S: Serialize> IpcSender<S> {
    /// Serialize and sent `value` through IPC channel.
    pub async fn send(&mut self, value: &S) -> Result<(), IpcError> {
        let msg_buf = bincode::serialize(value).map_err(|err| IpcError::SerializationError {
            reason: format!("{:?}", err),
        })?;
        // TODO: cleanup this casting here and on receive
        let msg_len_buf = (msg_buf.len() as i32).to_be_bytes();
        self.0
            .write_all(&msg_len_buf)
            .await
            .map_err(|err| IpcError::SendError {
                reason: err.to_string(),
            })?;
        self.0
            .write_all(&msg_buf)
            .await
            .map_err(|err| IpcError::SendError {
                reason: err.to_string(),
            })?;
        self.0.flush().await.map_err(|err| IpcError::SendError {
            reason: err.to_string(),
        })
    }
}

impl<S> Drop for IpcSender<S> {
    fn drop(&mut self) {
        if let Ok(tokio_runtime) = tokio::runtime::Handle::try_current() {
            // We only do this shutdown if the runtime is available, otherwise there is nothing to do.
            let _ = tokio::task::block_in_place(|| tokio_runtime.block_on(self.shutdown()).ok());
        }
    }
}

/// Represents receiving end of the IPC channel.
pub struct IpcReceiver<R>(OwnedReadHalf, PhantomData<R>);

impl<R> IpcReceiver<R>
where
    R: for<'de> Deserialize<'de>,
{
    /// Try to receive message with read_timeout,
    /// In case of timeout, can be IpcError::ReceiveMessageTimeout handled
    ///
    /// `read_timeout` - set read timeout before receive
    pub async fn try_receive(&mut self, read_timeout: Duration) -> Result<R, IpcError> {
        match tokio::time::timeout(read_timeout, self.receive()).await {
            Ok(result) => result,
            Err(_) => Err(IpcError::ReceiveMessageTimeout),
        }
    }

    /// Read bytes from established IPC channel and deserialize into a rust type.
    pub async fn receive(&mut self) -> Result<R, IpcError> {
        let mut msg_len_buf = [0; 4];
        self.0.read_exact(&mut msg_len_buf).await.map_err(|err| {
            IpcError::ReceiveMessageLengthError {
                reason: err.to_string(),
            }
        })?;

        let msg_len = i32::from_be_bytes(msg_len_buf) as usize;

        let mut msg_buf = vec![0u8; msg_len];
        self.0
            .read_exact(&mut msg_buf)
            .await
            .map_err(|err| IpcError::ReceiveMessageError {
                reason: err.to_string(),
            })?;

        bincode::deserialize(&msg_buf).map_err(|err| IpcError::DeserializationError {
            reason: format!("{:?}", err),
        })
    }
}

/// Listens for incoming IPC connections.
pub struct IpcServer<R, S> {
    listener: UnixListener,
    pub path: PathBuf,
    _phantom_r: PhantomData<R>,
    _phantom_s: PhantomData<S>,
}

impl<R, S> Drop for IpcServer<R, S> {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl<R, S> IpcServer<R, S>
where
    R: for<'de> Deserialize<'de>,
    S: Serialize,
{
    /// Bind IpcServer to random socket in temp folder
    pub fn bind() -> Result<Self, IpcError> {
        let path = temp_sock();
        Self::bind_path(&path)
    }

    /// Bind IpcServer to specific path
    ///
    /// # Arguments
    /// * `path` - path to the unix socket
    pub fn bind_path<P: AsRef<Path>>(path: P) -> Result<Self, IpcError> {
        let path_buf = path.as_ref().into();
        let listener = UnixListener::bind(path).map_err(|err| IpcError::ConnectionError {
            reason: err.to_string(),
        })?;

        Ok(IpcServer {
            listener,
            path: path_buf,
            _phantom_r: PhantomData,
            _phantom_s: PhantomData,
        })
    }

    /// Try to accept new connection a return sender/receiver for it
    /// In case of timeout, can be IpcError::AcceptTimeout handled
    ///
    /// `timeout` - timeout for wairing to a new connection
    ///
    /// Returns `blocking` receiver a `blocking` sender
    pub async fn try_accept(
        &mut self,
        timeout: Duration,
    ) -> Result<(IpcReceiver<R>, IpcSender<S>), IpcError> {
        let stream = match tokio::time::timeout(timeout, self.listener.accept()).await {
            Ok(Ok(connection)) => connection,
            Ok(Err(error)) => {
                return Err(IpcError::ConnectionError {
                    reason: error.to_string(),
                })
            }
            Err(_) => return Err(IpcError::AcceptTimeout { timeout }),
        };

        split(stream.0)
    }

    /// Accept new connection a return sender/receiver for it
    pub async fn accept(&mut self) -> Result<(IpcReceiver<R>, IpcSender<S>), IpcError> {
        let stream = self
            .listener
            .accept()
            .await
            .map_err(|e| IpcError::ConnectionError {
                reason: e.to_string(),
            })?;

        split(stream.0)
    }

    /// Create new IpcClient for this server
    pub fn client(&self) -> IpcClient<R, S> {
        IpcClient::new(&self.path)
    }
}

/// Connects to a listening IPC endpoint.
#[derive(Debug)]
pub struct IpcClient<R, S> {
    path: PathBuf,
    _phantom_r: PhantomData<R>,
    _phantom_s: PhantomData<S>,
}

impl<R, S> IpcClient<R, S> {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl<R, S> IpcClient<R, S>
where
    R: for<'de> Deserialize<'de>,
    S: Serialize,
{
    /// Create new client instance.
    ///
    /// # Arguments
    /// * `path` - path to existing unix socket
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        IpcClient {
            path: path.as_ref().into(),
            _phantom_r: PhantomData,
            _phantom_s: PhantomData,
        }
    }

    /// Try to open new connection.
    pub async fn connect(&self) -> Result<(IpcReceiver<R>, IpcSender<S>), IpcError> {
        let stream =
            UnixStream::connect(&self.path)
                .await
                .map_err(|err| IpcError::ConnectionError {
                    reason: err.to_string(),
                })?;
        split(stream)
    }
}

/// Crate new randomly named unix domain socket file in temp directory.
pub fn temp_sock() -> PathBuf {
    let mut rng = thread_rng();
    let temp_dir = env::temp_dir();
    let chars = iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .take(7)
        .collect::<String>();

    temp_dir.join(chars + ".sock")
}

fn split<R, S>(stream: UnixStream) -> Result<(IpcReceiver<R>, IpcSender<S>), IpcError>
where
    R: for<'de> Deserialize<'de>,
    S: Serialize,
{
    // TODO: use split, this allocates, split doesn't
    let (r, w) = stream.into_split();
    let receiver = IpcReceiver(r, PhantomData);
    let sender = IpcSender(w, PhantomData);

    Ok((receiver, sender))
}
