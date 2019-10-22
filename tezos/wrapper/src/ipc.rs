// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::env;
use std::fs;
use std::io;
use std::iter;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::time::Duration;

use failure::Fail;
use rand::{Rng, thread_rng};
use rand::distributions::Alphanumeric;
use serde::{Deserialize, Serialize};
use tokio::net::{UnixListener, UnixStream};
use tokio::prelude::*;
use tokio_io::split::{ReadHalf, WriteHalf};

const READ_TIMEOUT: Duration = Duration::from_secs(8);
const ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Fail)]
pub enum IpcError {
    #[fail(display = "Waiting for reply timed out")]
    ReceiveTimeout,
    #[fail(display = "Receive error: {}", reason)]
    ReceiveError {
        reason: io::Error
    },
    #[fail(display = "Send error: {}", reason)]
    SendError {
        reason: io::Error
    },
    #[fail(display = "Connection to socket timed out")]
    ConnectionTimeout,
    #[fail(display = "Waiting for client connection timed out")]
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
}

pub struct IpcSender<S>(WriteHalf<UnixStream>, PhantomData<S>);

impl<S: Serialize> IpcSender<S> {
    pub async fn send(&mut self, value: &S) -> Result<(), IpcError> {
        let msg_buf = bincode::serialize(value).map_err(|err| IpcError::SerializationError { reason: format!("{:?}", err) })?;
        let msg_len_buf = msg_buf.len().to_be_bytes();
        self.0.write_all(&msg_len_buf).await
            .map_err(|err| IpcError::SendError { reason: err })?;
        self.0.write_all(&msg_buf).await
            .map_err(|err| IpcError::SendError { reason: err })
    }
}

pub struct IpcReceiver<R>(ReadHalf<UnixStream>, PhantomData<R>);

impl<R> IpcReceiver<R>
where
    R: for<'de> Deserialize<'de>
{
    pub async fn receive(&mut self) -> Result<R, IpcError> {
        let mut msg_len_buf = [0; 8];
        self.0.read_exact(&mut msg_len_buf).timeout(READ_TIMEOUT).await
            .map_err(|_| IpcError::ReceiveTimeout)?
            .map_err(|err| IpcError::ReceiveError { reason: err })?;

        let msg_len = usize::from_be_bytes(msg_len_buf);

        let mut msg_buf = vec![0u8; msg_len];
        self.0.read_exact(&mut msg_buf).timeout(READ_TIMEOUT).await
            .map_err(|_| IpcError::ReceiveTimeout)?
            .map_err(|err| IpcError::ReceiveError { reason: err })?;

        bincode::deserialize(&msg_buf)
            .map_err(|err| IpcError::DeserializationError { reason: format!("{:?}", err) })
    }
}

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
    pub fn bind() -> Result<Self, IpcError> {
        let path = temp_sock();
        Self::bind_path(&path)
    }

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

    pub async fn accept(&mut self) -> Result<(IpcReceiver<R>, IpcSender<S>), IpcError> {
        let (stream, _) = self.listener.accept().timeout(ACCEPT_TIMEOUT).await
            .map_err(|_| IpcError::ConnectionTimeout)?
            .map_err(|err| IpcError::ConnectionError { reason: err })?;
        let (rx, tx) = tokio::io::split(stream);
        Ok((IpcReceiver(rx, PhantomData), IpcSender(tx, PhantomData)))
    }

    pub fn client(&self) -> IpcClient<R, S> {
        IpcClient::new(&self.path)
    }
}

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
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        IpcClient {
            path: path.as_ref().into(),
            _phantom_r: PhantomData,
            _phantom_s: PhantomData,
        }
    }

    pub async fn connect(&self) -> Result<(IpcReceiver<R>, IpcSender<S>), IpcError> {
        let stream = UnixStream::connect(&self.path).await.map_err(|err| IpcError::ConnectionError { reason: err })?;
        let (rx, tx) = tokio::io::split(stream);
        Ok((IpcReceiver(rx, PhantomData), IpcSender(tx, PhantomData)))
    }
}

pub fn temp_sock() -> PathBuf {
    let mut rng = thread_rng();
    let temp_dir = env::temp_dir();
    let chars = iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .take(7)
        .collect::<String>();

    temp_dir.join(chars + ".sock")
}




