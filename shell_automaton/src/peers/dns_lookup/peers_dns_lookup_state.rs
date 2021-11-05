// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::Port;

/// Different kinds of lookup errors that `getaddrinfo` and
/// `getnameinfo` can return. These can be a little inconsitant
/// between platforms, so it's recommended not to rely on them.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum DnsLookupError {
    /// Temporary failure in name resolution.
    ///
    /// May also be returend when DNS server returns a SERVFAIL.
    Again,
    /// Invalid value for `ai_flags' field.
    Badflags,
    /// NAME or SERVICE is unknown.
    ///
    /// May also be returned when domain doesn't exist (NXDOMAIN) or domain
    /// exists but contains no address records (NODATA).
    NoName,
    /// The specified network host exists, but has no data defined.
    ///
    /// This is no longer a POSIX standard, however it's still returned by
    /// some platforms. Be warned that FreeBSD does not include the corresponding
    /// `EAI_NODATA` symbol.
    NoData,
    /// Non-recoverable failure in name resolution.
    Fail,
    /// `ai_family' not supported.
    Family,
    /// `ai_socktype' not supported.
    Socktype,
    /// SERVICE not supported for `ai_socktype'.
    Service,
    /// Memory allocation failure.
    Memory,
    /// System error returned in `errno'.
    System,
    /// An unknown result code was returned.
    ///
    /// For some platforms, you may wish to match on an unknown value directly.
    /// Note that `gai_strerr` is used to get error messages, so the generated IO
    /// error should contain the correct error message for the platform.
    Unknown,
    /// A generic C error or IO error occured.
    ///
    /// You should convert this `LookupError` into an IO error directly. Note
    /// that the error code is set to 0 in the case this is returned.
    IO,
}

impl From<dns_lookup::LookupErrorKind> for DnsLookupError {
    fn from(err: dns_lookup::LookupErrorKind) -> Self {
        use dns_lookup::LookupErrorKind::*;

        match err {
            Again => Self::Again,
            Badflags => Self::Badflags,
            NoName => Self::NoName,
            NoData => Self::NoData,
            Fail => Self::Fail,
            Family => Self::Family,
            Socktype => Self::Socktype,
            Service => Self::Service,
            Memory => Self::Memory,
            System => Self::System,
            Unknown => Self::Unknown,
            IO => Self::IO,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeersDnsLookupStatus {
    Init,
    Success { addresses: Vec<SocketAddr> },
    Error { error: DnsLookupError },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersDnsLookupState {
    pub address: String,
    pub port: Port,
    pub status: PeersDnsLookupStatus,
}
