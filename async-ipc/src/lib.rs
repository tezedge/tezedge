// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! Provides Async IPC communication.
//!
//! The IPC is implemented as unix domain sockets. Functionality is similar to how network sockets work.
//!

use std::fs;
use std::iter;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::{env, thread};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// IPC communication errors
#[derive(Debug, Error)]
pub enum IpcError {
    #[error("Receive message length error: {reason}")]
    ReceiveMessageLengthError { reason: io::Error },
    #[error("Failed to spawn thread: {reason}")]
    ThreadError { reason: io::Error },
    #[error("Receive message error: {reason}")]
    ReceiveMessageError { reason: io::Error },
    #[error("Send error: {reason}")]
    SendError { reason: io::Error },
    #[error("Accept connection timed out, timout: {timeout:?}")]
    AcceptTimeout { timeout: Duration },
    #[error("Receive message timed out - handle WouldBlock scenario if needed")]
    ReceiveMessageTimeout,
    #[error("Discard message timed out")]
    DiscardMessageTimeout,
    #[error("Connection error: {reason}")]
    ConnectionError { reason: io::Error },
    #[error("Serialization error: {reason}")]
    SerializationError { reason: String },
    #[error("Deserialization error: {reason}")]
    DeserializationError { reason: String },
    #[error("Split stream error: {reason}")]
    SplitError { reason: io::Error },
    #[error("Socker configuration error: {reason}")]
    SocketConfigurationError { reason: io::Error },
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
        self.0.shutdown().await?;
        Ok(())
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
            .map_err(|err| IpcError::SendError { reason: err })?;
        self.0
            .write_all(&msg_buf)
            .await
            .map_err(|err| IpcError::SendError { reason: err })?;
        self.0
            .flush()
            .await
            .map_err(|err| IpcError::SendError { reason: err })
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
    /// `reset_read_timeout` - set read timeout after receive
    pub async fn try_receive(&mut self, read_timeout: Option<Duration>) -> Result<R, IpcError> {
        // TODO: add timeouts
        let receive_result = self.receive().await;
        receive_result
    }

    /// Read bytes from established IPC channel and deserialize into a rust type.
    pub async fn receive(&mut self) -> Result<R, IpcError> {
        let mut msg_len_buf = [0; 4];
        self.0.read_exact(&mut msg_len_buf).await.map_err(|err| {
            if err.kind() == io::ErrorKind::WouldBlock {
                IpcError::ReceiveMessageTimeout
            } else {
                IpcError::ReceiveMessageLengthError { reason: err }
            }
        })?;

        let msg_len = i32::from_be_bytes(msg_len_buf) as usize;

        let mut msg_buf = vec![0u8; msg_len];
        self.0
            .read_exact(&mut msg_buf)
            .await
            .map_err(|err| IpcError::ReceiveMessageError { reason: err })?;

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
        let listener =
            UnixListener::bind(path).map_err(|err| IpcError::ConnectionError { reason: err })?;

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
        // very simple retry logic
        // TODO: revise this, timeout doesn't work like that in tokio
        let deadline = Instant::now() + timeout;
        let stream = loop {
            match self.listener.accept().await {
                Ok(connection) => break connection,
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if deadline.checked_duration_since(Instant::now()).is_some() {
                        // ok, lets retry
                        thread::sleep(Duration::from_millis(50))
                    } else {
                        // deadline is over
                        return Err(IpcError::AcceptTimeout { timeout });
                    }
                }
                Err(e) => {
                    return Err(IpcError::ConnectionError { reason: e });
                }
            }
        };

        split(stream.0)
    }

    /// Accept new connection a return sender/receiver for it
    pub async fn accept(&mut self) -> Result<(IpcReceiver<R>, IpcSender<S>), IpcError> {
        let stream = self
            .listener
            .accept()
            .await
            .map_err(|e| IpcError::ConnectionError { reason: e })?;

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
        let stream = UnixStream::connect(&self.path)
            .await
            .map_err(|err| IpcError::ConnectionError { reason: err })?;
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
