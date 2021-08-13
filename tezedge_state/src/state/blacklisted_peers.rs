use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, SystemTime};

use crypto::hash::CryptoboxPublicKeyHash;

use crate::{PeerAddress, Port};

#[derive(Debug, Clone)]
pub struct BlacklistedPeer {
    pub since: SystemTime,
    pub port: Option<Port>,
}

impl BlacklistedPeer {
    pub fn is_expired(&self, at: SystemTime, expiary_duration: Duration) -> bool {
        at.duration_since(self.since)
            .ok()
            .map(|time_passed| time_passed >= expiary_duration)
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub struct BlacklistedPeers {
    ips: HashMap<IpAddr, BlacklistedPeer>,
}

impl BlacklistedPeers {
    pub fn new() -> Self {
        Self {
            ips: HashMap::new(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.ips.len()
    }

    pub fn insert_ip(&mut self, ip: IpAddr, peer: BlacklistedPeer) {
        self.ips.insert(ip, peer);
    }

    /// Check if ip address is blacklisted.
    pub fn is_address_blacklisted(&self, addr: &PeerAddress) -> bool {
        self.ips.contains_key(&addr.ip())
    }

    pub fn is_identity_blacklisted(&self, _identity: CryptoboxPublicKeyHash) -> bool {
        // TODO
        false
    }

    /// Whitelist and return expired blacklisted peers.
    pub(crate) fn take_expired_blacklisted_peers(
        &mut self,
        at: SystemTime,
        expiary_duration: Duration,
    ) -> Vec<(IpAddr, Option<Port>)> {
        let addrs = self
            .ips
            .iter()
            .filter(|(_, x)| x.is_expired(at, expiary_duration))
            .map(|(ip, peer)| (*ip, peer.port))
            .collect::<Vec<_>>();

        for addr in addrs.iter() {
            self.ips.remove(&addr.0);
        }

        addrs
    }
}
