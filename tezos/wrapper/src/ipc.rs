// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

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

const READ_TIMEOUT: Duration = Duration::from_secs(8);
const ACCEPT_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Debug, Fail)]
pub enum IpcError {
    #[fail(display = "Receive error: {}", reason)]
    ReceiveError {
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
    }
}

pub struct IpcSender<S>(UnixStream, PhantomData<S>);

impl<S: Serialize> IpcSender<S> {
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

    pub fn shutdown(&self) -> Result<(), io::Error> {
        self.0.shutdown(Shutdown::Write)
    }
}

pub struct IpcReceiver<R>(UnixStream, PhantomData<R>);

impl<R> IpcReceiver<R>
where
    R: for<'de> Deserialize<'de>
{
    pub fn receive(&mut self) -> Result<R, IpcError> {
        let mut msg_len_buf = [0; 8];
        self.0.read_exact(&mut msg_len_buf)
            .map_err(|err| IpcError::ReceiveError { reason: err })?;

        let msg_len = usize::from_be_bytes(msg_len_buf);

        let mut msg_buf = vec![0u8; msg_len];
        self.0.read_exact(&mut msg_buf)
            .map_err(|err| IpcError::ReceiveError { reason: err })?;

        bincode::deserialize(&msg_buf)
            .map_err(|err| IpcError::DeserializationError { reason: format!("{:?}", err) })
    }

    pub fn shutdown(&self) -> Result<(), io::Error> {
        self.0.shutdown(Shutdown::Read)
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

    pub fn accept(&mut self) -> Result<(IpcReceiver<R>, IpcSender<S>), IpcError> {
        let stream = self.listener.try_accept(ACCEPT_TIMEOUT)
            .map_err(|_| IpcError::AcceptTimeout)?;
        split(stream, Some(READ_TIMEOUT)).map_err(|err| IpcError::SplitError { reason: err })
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

    pub fn connect(&self) -> Result<(IpcReceiver<R>, IpcSender<S>), IpcError> {
        let stream = UnixStream::connect(&self.path).map_err(|err| IpcError::ConnectionError { reason: err })?;
        split(stream, None).map_err(|err| IpcError::SplitError { reason: err })
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

fn split<R, S>(stream: UnixStream, read_timeout: Option<Duration>) -> Result<(IpcReceiver<R>, IpcSender<S>), io::Error>
where
    R: for<'de> Deserialize<'de>,
    S: Serialize
{
    stream.set_read_timeout(read_timeout)?;
    Ok((IpcReceiver(stream.try_clone()?, PhantomData), IpcSender(stream, PhantomData)))
}



