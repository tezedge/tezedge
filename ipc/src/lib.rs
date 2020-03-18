// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Provides IPC communication.
//!
//! The IPC is implemented as unix domain sockets. Functionality is similar to how network sockets work.

use std::env;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::iter;
use std::marker::PhantomData;
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use failure::Fail;
use rand::{Rng, thread_rng};
use rand::distributions::Alphanumeric;
use serde::{Deserialize, Serialize};
use timeout_io::Acceptor;

/// IPC communication errors
#[derive(Debug, Fail)]
pub enum IpcError {
    #[fail(display = "Receive message length error: {}", reason)]
    ReceiveMessageLengthError {
        reason: io::Error
    },
    #[fail(display = "Receive message error: {}", reason)]
    ReceiveMessageError {
        reason: io::Error
    },
    #[fail(display = "Send error: {}", reason)]
    SendError {
        reason: io::Error
    },
    #[fail(display = "Accept connection timed out")]
    AcceptTimeout,
    #[fail(display = "Connection error: {}", reason)]
    ConnectionError {
        reason: io::Error,
    },
    #[fail(display = "Serialization error: {}", reason)]
    SerializationError {
        reason: String
    },
    #[fail(display = "Deserialization error: {}", reason)]
    DeserializationError {
        reason: String
    },
    #[fail(display = "Split stream error: {}", reason)]
    SplitError {
        reason: io::Error
    },
    #[fail(display = "Socker configuration error: {}", reason)]
    SocketConfigurationError {
        reason: io::Error,
    },
}

/// Represents sending end of the IPC channel.
pub struct IpcSender<S>(UnixStream, PhantomData<S>);

impl<S> IpcSender<S> {
    /// Close IPC channel and release associated resources.
    ///
    /// This closes only the sending part of the IPC channel.
    pub fn shutdown(&self) -> Result<(), io::Error> {
        self.0.shutdown(Shutdown::Write)
    }

    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_write_timeout(timeout)
    }
}

impl<S: Serialize> IpcSender<S> {
    /// Serialize and sent `value` through IPC channel.
    ///
    /// This is a blocking operation,
    pub fn send(&mut self, value: &S) -> Result<(), IpcError> {
        let msg_buf = bincode::serialize(value).map_err(|err| IpcError::SerializationError { reason: format!("{:?}", err) })?;
        let msg_len_buf = msg_buf.len().to_be_bytes();
        self.0.write_all(&msg_len_buf)
            .map_err(|err| IpcError::SendError { reason: err })?;
        self.0.write_all(&msg_buf)
            .map_err(|err| IpcError::SendError { reason: err })?;
        self.0.flush()
            .map_err(|err| IpcError::SendError { reason: err })
    }
}

impl<S> Drop for IpcSender<S> {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// Represents receiving end of the IPC channel.
pub struct IpcReceiver<R>(UnixStream, PhantomData<R>);

impl<R> IpcReceiver<R> {
    /// Close IPC channel and release associated resources.
    ///
    /// This closes only the receiving part of the IPC channel.
    pub fn shutdown(&self) -> Result<(), io::Error> {
        self.0.shutdown(Shutdown::Read)
    }

    /// Set read timeout
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_read_timeout(timeout)
    }
}

impl<R> IpcReceiver<R>
where
    R: for<'de> Deserialize<'de>
{
    /// Read bytes from established IPC channel and deserialize into a rust type.
    pub fn receive(&mut self) -> Result<R, IpcError> {
        let mut msg_len_buf = [0; 8];
        self.0.read_exact(&mut msg_len_buf)
            .map_err(|err| IpcError::ReceiveMessageLengthError { reason: err })?;

        let msg_len = usize::from_be_bytes(msg_len_buf);

        let mut msg_buf = vec![0u8; msg_len];
        self.0.read_exact(&mut msg_buf)
            .map_err(|err| IpcError::ReceiveMessageError { reason: err })?;

        bincode::deserialize(&msg_buf)
            .map_err(|err| IpcError::DeserializationError { reason: format!("{:?}", err) })
    }
}

impl<R> Drop for IpcReceiver<R> {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// Listens for incoming IPC connections.
pub struct IpcServer<R, S> {
    listener: UnixListener,
    path: PathBuf,
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
    S: Serialize
{
    const ACCEPT_TIMEOUT: Duration = Duration::from_secs(3);

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
        let listener = UnixListener::bind(path)
            .map_err(|err| IpcError::ConnectionError { reason: err })?;

        Ok(IpcServer {
            listener,
            path: path_buf,
            _phantom_r: PhantomData,
            _phantom_s: PhantomData,
        })
    }

    /// Accept new connection a return sender/receiver for it
    pub fn accept(&mut self) -> Result<(IpcReceiver<R>, IpcSender<S>), IpcError> {
        let stream = self.listener.try_accept(Self::ACCEPT_TIMEOUT)
            .map_err(|_| IpcError::AcceptTimeout)?;
        split(stream).map_err(|err| IpcError::SplitError { reason: err })
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
        S: Serialize
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
    pub fn connect(&self) -> Result<(IpcReceiver<R>, IpcSender<S>), IpcError> {
        let stream = UnixStream::connect(&self.path).map_err(|err| IpcError::ConnectionError { reason: err })?;
        split(stream).map_err(|err| IpcError::SplitError { reason: err })
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

fn split<R, S>(stream: UnixStream) -> Result<(IpcReceiver<R>, IpcSender<S>), io::Error>
    where
        R: for<'de> Deserialize<'de>,
        S: Serialize
{
    Ok((IpcReceiver(stream.try_clone()?, PhantomData), IpcSender(stream, PhantomData)))
}



