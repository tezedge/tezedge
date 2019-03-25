use std::collections::HashMap;
use std::sync::Arc;

use crate::tezos::p2p::peer::P2pPeer;

/// Structure for holding live references to all authenticated peers,
/// for further user, e.g. send bootstrap to all peer
pub struct P2pPool {
    pub peers: HashMap<String, Arc<P2pPeer>>
}

impl P2pPool {
    pub fn new() -> Self {
        P2pPool {
            peers: HashMap::new()
        }
    }

    pub fn get_network_peer_as_json(&self) -> String {
        let mut peers_as_json = vec![];

        for (_, p2p_peer) in self.peers.iter() {
            let ip = &p2p_peer.get_peer().host.to_string();
            let port = p2p_peer.get_peer().port;
            peers_as_json.push(format!("[\"{}:{:?}\"]", ip, port));
        }

        let peers_as_json = peers_as_json.join(",");
        peers_as_json
    }
}