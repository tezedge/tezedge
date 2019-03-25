use serde::{Deserialize};

#[derive(Debug, Deserialize)]
pub struct PeerURL {
    pub host: String,
    pub port: u16
}

#[derive(Debug, Deserialize)]
pub struct BootstrapMessage {
    pub initial_peers: Vec<PeerURL>,
}

#[derive(Debug, Deserialize)]
pub struct EmptyMessage {
}

#[derive(Debug, Deserialize)]
#[serde(tag = "msg")]
pub enum RpcMessage {
    #[serde(rename = "bootstrapWithPeers")]
    BootstrapWithPeers(BootstrapMessage),
    #[serde(rename = "bootstrapWithLookup")]
    BootstrapWithLookup(EmptyMessage),
    NetworkPoints(EmptyMessage),
    ChainsHead(EmptyMessage),
}
