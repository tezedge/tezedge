use std::collections::HashMap;
use std::sync::Arc;

use super::peer::Peer;
use crate::rpc::message::PeerAddress;

/// Structure for holding live references to all authenticated peers,
/// for further user, e.g. send bootstrap to all peer
pub struct P2pPool {
    peers: HashMap<String, Arc<P2pPeer>>
}

impl P2pPool {

    pub fn new() -> Self {
        P2pPool {
            peers: HashMap::new()
        }
    }

    pub fn get_peers_addresses(&self) -> Vec<PeerAddress> {
        let mut addresses = vec![];
        for val in self.peers.values() {
            addresses.push(val.get_peer().clone())
        }
        addresses
    }

    pub fn insert_peer(&mut self, peer_id: &str, peer: Arc<Peer>) {
        self.peers.insert(String::from(peer_id), peer);
    }
}