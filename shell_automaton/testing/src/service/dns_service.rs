// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use shell_automaton::service::dns_service::{DnsLookupError, DnsService};

#[derive(Debug, Clone)]
pub enum DnsServiceMocked {
    Constant(Result<Vec<SocketAddr>, DnsLookupError>),
}

impl DnsService for DnsServiceMocked {
    fn resolve_dns_name_to_peer_address(
        &mut self,
        _: &str,
        _: u16,
    ) -> Result<Vec<SocketAddr>, DnsLookupError> {
        match self {
            DnsServiceMocked::Constant(res) => res.clone(),
        }
    }
}
