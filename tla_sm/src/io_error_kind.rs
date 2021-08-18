// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::io::ErrorKind;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum IOErrorKind {
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    ConnectionAborted,
    NotConnected,
    AddrInUse,
    AddrNotAvailable,
    BrokenPipe,
    AlreadyExists,
    WouldBlock,
    InvalidInput,
    InvalidData,
    TimedOut,
    WriteZero,
    Interrupted,
    Other,
    UnexpectedEof,
}

impl From<ErrorKind> for IOErrorKind {
    fn from(err: ErrorKind) -> Self {
        match err {
            ErrorKind::NotFound => Self::NotFound,
            ErrorKind::PermissionDenied => Self::PermissionDenied,
            ErrorKind::ConnectionRefused => Self::ConnectionRefused,
            ErrorKind::ConnectionReset => Self::ConnectionReset,
            ErrorKind::ConnectionAborted => Self::ConnectionAborted,
            ErrorKind::NotConnected => Self::NotConnected,
            ErrorKind::AddrInUse => Self::AddrInUse,
            ErrorKind::AddrNotAvailable => Self::AddrNotAvailable,
            ErrorKind::BrokenPipe => Self::BrokenPipe,
            ErrorKind::AlreadyExists => Self::AlreadyExists,
            ErrorKind::WouldBlock => Self::WouldBlock,
            ErrorKind::InvalidInput => Self::InvalidInput,
            ErrorKind::InvalidData => Self::InvalidData,
            ErrorKind::TimedOut => Self::TimedOut,
            ErrorKind::WriteZero => Self::WriteZero,
            ErrorKind::Interrupted => Self::Interrupted,
            ErrorKind::Other => Self::Other,
            ErrorKind::UnexpectedEof => Self::UnexpectedEof,
            _ => Self::Other,
        }
    }
}

impl From<IOErrorKind> for ErrorKind {
    fn from(err: IOErrorKind) -> Self {
        match err {
            IOErrorKind::NotFound => Self::NotFound,
            IOErrorKind::PermissionDenied => Self::PermissionDenied,
            IOErrorKind::ConnectionRefused => Self::ConnectionRefused,
            IOErrorKind::ConnectionReset => Self::ConnectionReset,
            IOErrorKind::ConnectionAborted => Self::ConnectionAborted,
            IOErrorKind::NotConnected => Self::NotConnected,
            IOErrorKind::AddrInUse => Self::AddrInUse,
            IOErrorKind::AddrNotAvailable => Self::AddrNotAvailable,
            IOErrorKind::BrokenPipe => Self::BrokenPipe,
            IOErrorKind::AlreadyExists => Self::AlreadyExists,
            IOErrorKind::WouldBlock => Self::WouldBlock,
            IOErrorKind::InvalidInput => Self::InvalidInput,
            IOErrorKind::InvalidData => Self::InvalidData,
            IOErrorKind::TimedOut => Self::TimedOut,
            IOErrorKind::WriteZero => Self::WriteZero,
            IOErrorKind::Interrupted => Self::Interrupted,
            IOErrorKind::Other => Self::Other,
            IOErrorKind::UnexpectedEof => Self::UnexpectedEof,
        }
    }
}
