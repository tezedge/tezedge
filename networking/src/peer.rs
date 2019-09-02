use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures::lock::Mutex;
use riker::actors::*;

use crate::network_channel::NetworkChannelMsg;
use crate::p2p::stream::{MessageReader, MessageWriter};
use crate::p2p::client::Version;
use crate::p2p::encoding::connection::ConnectionMessage;
use crypto::nonce::Nonce;
use crate::p2p::{
    encoding::prelude::*,
    message::{BinaryMessage, RawBinaryMessage, JsonMessage},
    peer::{P2pPeer, PeerState},
    stream::MessageStream,
};
use crypto::nonce;
use std::convert::TryFrom;
use crypto::crypto_box::PrecomputedKey;
use crypto::{
    crypto_box::*,
    nonce::*
};
use std::net::SocketAddr;

static ACTOR_ID_GENERATOR: AtomicU64 = AtomicU64::new(0);
static VERSIONS: Vec<Version> = vec![Version { name: String::from("TEZOS_ALPHANET_2018-11-30T15:30:56Z"), major: 0, minor: 0 }];

pub type PeerId = String;
pub type PublicKey = Vec<u8>;

/// Message sender encapsulates process of the outgoing message transmission.
/// This process involves (not only) nonce increment, encryption and network transmission.
struct MessageSender {
    /// Precomputed key is created from merge of peer public key and our secret key.
    /// It's used to speedup of crypto operations.
    precomputed_key: PrecomputedKey,

    /// Nonce used to encrypt outgoing messages
    nonce_local: Nonce,

    /// Outgoing message writer
    tx: MessageWriter,
}

impl MessageSender {
    pub async fn send<'a>(&'a mut self, message: &'a impl BinaryMessage) -> Result<(), std::io::Error> {
        let message_bytes = message.as_bytes()?;
        if self.log_messages {
            debug!("Message to send to peer {} as hex (without length): \n{}", self.peer_id, hex::encode(&message_bytes));
        } else {
            debug!("Message to send to peer {} ...", self.peer_id);
        }

        // encrypt
        let message_encrypted = encrypt(
            &message_bytes,
            &self.nonce_local,
            &self.precomputed_key,
        )?;
        if self.log_messages {
            debug!("Message (enc) to send to peer {} as hex (without length): \n{}", self.peer_id, hex::encode(&message_encrypted));
        }

        // increment nonce
        self.increment_nonce_local();

        // send
        let mut tx = self.tx.lock().await;
        if let Err(e) = tx.write_message(&message_encrypted).await {
            bail!("Failed to transfer encoding: {:?}", e);
        }

        Ok(())
    }

    fn increment_nonce_local(&mut self) {
        self.nonce_local = self.nonce_local.increment();
    }
}

struct MessageReceiver {
    /// Precomputed key is created from merge of peer public key and our secret key.
    /// It's used to speedup of crypto operations.
    precomputed_key: PrecomputedKey,

    nonce_remote: Nonce,

    rx: MessageReader,
}

impl MessageReceiver {
    pub async fn receive(&mut self) -> Result<Vec<u8>, std::io::Error> {
        // read
        let mut rx = self.rx.lock().await;
        let message_encrypted = rx.read_message().await?;

        // increment nonce
        let remote_nonce_to_use = self.unwrap().nonce_remote.clone();
        self.peer_state.write().unwrap().increment_nonce_remote();

        // decrypt
        match decrypt(message_encrypted.get_contents(), &remote_nonce_to_use, &self.precomputed_key) {
            Ok(message) => {
                if self.log_messages {
                    debug!("Message received from peer {} as hex: \n{}", self.peer_id, hex::encode(&message));
                } else {
                    debug!("Message received from peer {} ... ", self.peer_id);
                }
                Ok(message)
            }
            Err(ref e) => {
                bail!("Decrypt encoding failed from peer: {} Reason: {:?}", self.peer_id, e)
            }
        }
    }

    fn increment_nonce_remote(&mut self) {
        self.nonce_remote = self.nonce_remote.increment();
    }
}

/// P2pPeer represents live secured connection with remote peer
///
/// Network layer communicates with remote peer through TCP/IP.
/// Every peer has it's own identifier represented by hash of it's public key.
struct PeerInternal {
    /// Peer ID is created as hex string representation of peer public key bytes.
    peer_id: PeerId,
    /// Peer ip/port
    address: SocketAddr,
    /// Peer public key
    public_key: PublicKey,

    /// Message sender
    tx: Arc<Mutex<MessageSender>>,
    /// Message receiver
    rx: Arc<Mutex<MessageReceiver>>,

    /// print messages as hex string in log statements
    log_messages: bool,
}

#[derive(Clone, Debug)]
pub struct Bootstrap {
    pub rx: Arc<Mutex<MessageReader>>,
    pub tx: Arc<Mutex<MessageWriter>>,
    pub address: SocketAddr,
    pub listener_port: u16,
    pub public_key: String,
    pub secret_key: String,
    pub proof_of_work_stamp: String,
    pub incoming: bool,
}

#[derive(Clone, Debug)]
pub struct Terminate;


#[actor(Bootstrap, Terminate)]
pub struct Peer {
    event_channel: ChannelRef<NetworkChannelMsg>,
}

pub type PeerRef = ActorRef<PeerMsg>;

impl Peer {
    pub fn new(sys: &ActorSystem, net_chan: ChannelRef<NetworkChannelMsg>) -> Result<PeerRef, CreateError> {
        let actor_id = ACTOR_ID_GENERATOR.fetch_add(1, Ordering::SeqCst);
        sys.actor_of(Peer::props(net_chan), &format!("peer-{}", actor_id))
    }

    fn props(net_chan: ChannelRef<NetworkChannelMsg>) -> BoxActorProd<Peer> {
        Props::new_args(Peer::actor, net_chan)
    }

    fn actor(message_channel: ChannelRef<NetworkChannelMsg>) -> Self {
        Peer { event_channel: message_channel, }
    }

}


impl Actor for Peer {
    type Msg = PeerMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        // Use the respective Receive<T> implementation
        self.receive(ctx, msg, sender);
    }
}

impl Receive<Bootstrap> for Peer {
    type Msg = PeerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Bootstrap, sender: Sender) {
        tokio::spawn(async move {

            // send connection encoding
            let msg_tx = msg.tx.lock().await;
            let connection_message: ConnectionMessage = msg.into();
            let connection_message_sent = {
                let as_bytes = connection_message.as_bytes()?;
                msg_tx.write_message(&as_bytes).await
            };
            if let Err(e) = connection_message_sent {
                bail!("Failed to transfer bootstrap encoding: {:?}", e);
            }
            let connection_msg_bytes_sent = connection_message_sent.unwrap();

            // receive connection message
            let msg_rx = msg.rx.lock().await;
            let received_connection_msg = msg_rx.read_message().await;
            if let Err(e) = received_connection_msg {
                bail!("Failed to receive response to our bootstrap encoding: {:?}", e);
            }
            let received_connection_msg = received_connection_msg.unwrap();
            // generate local and remote nonce
            let (nonce_local, nonce_remote) = generate_nonces(&connection_msg_bytes_sent, &received_connection_msg, msg.incoming);

            // convert received bytes from remote peer into `ConnectionMessage`
            let received_connection_msg: ConnectionMessage = received_connection_msg.try_into()?;
            let peer_public_key = received_connection_msg.get_public_key();
            debug!("Received peer_public_key: {}", hex::encode(&peer_public_key));

            let peer = P2pPeer::new(
                peer_public_key.clone(),
                &self.identity.secret_key,
                PeerAddress { host: peer.host.clone(), port: peer.port },
                PeerState::new(
                    &nonce_local,
                    &nonce_remote),
                msg_rx,
                msg_tx,
                self.log_messages,
            );
        });
    }
}

impl Receive<Terminate> for Peer {
    type Msg = PeerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Terminate, sender: Sender) {
        unimplemented!()
    }
}


/// Generate nonces (sent and recv encoding must be with length bytes also)
///
/// local_nonce is used for writing crypto messages to other peers
/// remote_nonce is used for reading crypto messages from other peers
fn generate_nonces(sent_msg: &RawBinaryMessage, recv_msg: &RawBinaryMessage, incoming: bool) -> (Nonce, Nonce) {
    nonce::generate_nonces(sent_msg.get_raw(), recv_msg.get_raw(), incoming)
}

impl From<Bootstrap> for ConnectionMessage {
    fn from(msg: Bootstrap) -> Self {
        // generate init random nonce
        let nonce = Nonce::random();
        ConnectionMessage::new(
            msg.listener_port,
            &msg.identity.public_key,
            &msg.identity.proof_of_work_stamp,
            &nonce.get_bytes(),
            VERSIONS.iter().map(|v| v.into()).collect(),
        )
    }
}
