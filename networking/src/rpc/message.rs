use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeerAddress {
    pub host: String,
    pub port: u16
}

#[derive(Debug, Deserialize)]
pub struct BootstrapMessage {
    pub initial_peers: Vec<PeerAddress>,
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
