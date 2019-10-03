use serde::{Serialize, Deserialize};
use networking::p2p::{
    network_channel::NetworkChannelMsg,
};
use riker::actor::ActorReference;
use std::time::SystemTime;
use std::borrow::Borrow;
use networking::p2p::encoding::peer::PeerMessageResponse;

#[derive(Serialize, Deserialize)]
pub enum Record {
    PeerCreated {
        ts: f64,
        peer_id: String,
    },
    PeerBootstrapped {
        ts: f64,
        peer_id: String,
        public_key: String,
    },
    PeerReceivedMessage {
        ts: f64,
        peer_id: String,
        content: Vec<u8>,
    },
}

impl From<NetworkChannelMsg> for Record {
    fn from(msg: NetworkChannelMsg) -> Self {
        let ts: f64 = SystemTime::UNIX_EPOCH.elapsed()
            .expect("UNIX_EPOCH fatal failure").as_secs_f64();

        match msg {
            NetworkChannelMsg::PeerCreated(msg) => Record::PeerCreated {
                ts,
                peer_id: msg.peer.uri().name.to_string(),
            },
            NetworkChannelMsg::PeerBootstrapped(msg) => Record::PeerBootstrapped {
                ts,
                peer_id: msg.peer.uri().name.to_string(),
                public_key: msg.peer_id,
            },
            NetworkChannelMsg::PeerMessageReceived(msg) => Record::PeerReceivedMessage {
                ts,
                peer_id: msg.peer.uri().name.to_string(),
                content: serde_cbor::to_vec::<PeerMessageResponse>(msg.message.borrow())
                    .expect("Failed to serialize message content"),
            }
        }
    }
}