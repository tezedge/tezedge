use std::convert::TryFrom;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use failure::_core::sync::atomic::AtomicBool;
use failure::Fail;
use futures::lock::Mutex;
use log::{debug, info, warn};
use riker::actors::*;
use tokio::net::TcpStream;

use crypto::crypto_box::*;
use crypto::nonce::{self, Nonce, NoncePair};
use tezos_encoding::hash::ChainId;

use super::encoding::prelude::*;
use super::message::{BinaryMessage, RawBinaryMessage};
use super::network_channel::{NetworkChannelTopic, NetworkChannelMsg, PeerBootstrapped, PeerMessageReceived};
use super::stream::{MessageReader, MessageStream, MessageWriter};
use crate::p2p::network_channel::PeerCreated;

static ACTOR_ID_GENERATOR: AtomicU64 = AtomicU64::new(0);

pub type PeerId = String;
pub type PublicKey = Vec<u8>;

#[derive(Debug, Fail)]
enum PeerError {
    #[fail(display = "Received NACK from remote peer")]
    NackReceived,
    #[fail(display = "Failed to create precomputed key")]
    FailedToPrecomputeKey,
    #[fail(display = "Network error: {}", message)]
    NetworkError {
        message: &'static str,
        error: io::Error,
    },
    #[fail(display = "Failed to decrypt message")]
    FailedToDecryptMessage {
        error: CryptoError
    },
    #[fail(display = "Failed to encrypt message")]
    FailedToEncryptMessage {
        error: CryptoError
    },
    #[fail(display = "Message serialization error")]
    SerializationError {
        error: tezos_encoding::ser::Error
    },
    #[fail(display = "Message deserialization error")]
    DeserializationError {
        error: tezos_encoding::de::Error
    }
}

impl From<tezos_encoding::ser::Error> for PeerError {
    fn from(error: tezos_encoding::ser::Error) -> Self {
        PeerError::SerializationError { error }
    }
}

impl From<tezos_encoding::de::Error> for PeerError {
    fn from(error: tezos_encoding::de::Error) -> Self {
        PeerError::DeserializationError { error }
    }
}

impl From<std::io::Error> for PeerError {
    fn from(error: std::io::Error) -> Self {
        PeerError::NetworkError { error, message: "Network IO error" }
    }
}

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
    /// Peer ID is created as hex string representation of peer public key bytes.
    peer_id: PeerId,
}

impl MessageSender {
    pub async fn write_message<'a>(&'a mut self, message: &'a impl BinaryMessage) -> Result<(), PeerError> {
        let message_bytes = message.as_bytes()?;
        debug!("Message to send to peer {} as hex (without length): \n{}", self.peer_id, hex::encode(&message_bytes));

        // encrypt
        let message_encrypted = match encrypt(&message_bytes, &self.nonce_fetch_increment(), &self.precomputed_key) {
            Ok(msg) => msg,
            Err(error) => return Err(PeerError::FailedToEncryptMessage { error })
        };
        debug!("Message (enc) to send to peer {} as hex (without length): \n{}", self.peer_id, hex::encode(&message_encrypted));

        // send
        self.tx.write_message(&message_encrypted).await?;

        Ok(())
    }

    fn nonce_fetch_increment(&mut self) -> Nonce {
        let incremented = self.nonce_local.increment();
        std::mem::replace(&mut self.nonce_local, incremented)
    }
}

struct MessageReceiver {
    /// Precomputed key is created from merge of peer public key and our secret key.
    /// It's used to speedup of crypto operations.
    precomputed_key: PrecomputedKey,
    /// Nonce used to decrypt received messages
    nonce_remote: Nonce,
    /// Incoming message reader
    rx: MessageReader,
    /// Peer ID is created as hex string representation of peer public key bytes.
    peer_id: PeerId,
}

impl MessageReceiver {
    pub async fn read_message(&mut self) -> Result<Vec<u8>, PeerError> {
        // read
        let message_encrypted = self.rx.read_message().await?;

        // decrypt
        match decrypt(message_encrypted.get_contents(), &self.nonce_fetch_increment(), &self.precomputed_key) {
            Ok(message) => {
                debug!("Message received from peer {} as hex: \n{}", self.peer_id, hex::encode(&message));
                Ok(message)
            }
            Err(error) => {
                Err(PeerError::FailedToDecryptMessage { error })
            }
        }
    }

    fn nonce_fetch_increment(&mut self) -> Nonce {
        let incremented = self.nonce_remote.increment();
        std::mem::replace(&mut self.nonce_remote, incremented)
    }
}

#[derive(Clone, Debug)]
pub struct Bootstrap {
    stream: Arc<Mutex<Option<TcpStream>>>,
    address: SocketAddr,
    incoming: bool,
}

impl Bootstrap {
    pub fn incoming(stream: TcpStream, address: SocketAddr) -> Self {
        Bootstrap { stream: Arc::new(Mutex::new(Some(stream))), address, incoming: true }
    }

    pub fn outgoing(stream: TcpStream, address: SocketAddr) -> Self {
        Bootstrap { stream: Arc::new(Mutex::new(Some(stream))), address, incoming: false }
    }
}

#[derive(Clone, Debug)]
pub struct GetCurrentBranch {
    chain_id: ChainId
}

#[derive(Clone, Debug)]
pub struct SendMessage {
    /// Message is wrapped in `Arc` to avoid excessive cloning.
    message: Arc<PeerMessageResponse>
}

impl SendMessage {
    pub fn new(msg: PeerMessageResponse) -> Self {
        SendMessage { message: Arc::new(msg) }
    }
}

#[derive(Clone)]
struct Network {
    /// Message receiver boolean indicating whether
    /// more messages should be received from network
    rx_run: AtomicBool,
    /// Message sender
    tx: Arc<Mutex<Option<MessageSender>>>,
}

/// Local node info
pub struct Local {
    /// port where remote node can establish new connection
    listener_port: u16,
    /// our public key
    public_key: String,
    /// our secret key
    secret_key: String,
    /// proof of work
    proof_of_work_stamp: String,
}

pub type PeerRef = ActorRef<PeerMsg>;

#[actor(Bootstrap, GetCurrentBranch)]
pub struct Peer {
    /// All events generated by the peer will end up in this channel
    event_channel: ChannelRef<NetworkChannelMsg>,
    /// Local node info
    local: Arc<Local>,
    /// Network IO
    net: Network,
}

impl Peer {

    pub fn new(sys: &ActorSystem,
               event_channel: ChannelRef<NetworkChannelMsg>,
               address: &SocketAddr,
               listener_port: u16,
               public_key: &String,
               secret_key: &String,
               proof_of_work_stamp: &String) -> Result<PeerRef, CreateError>
    {
        let info = Local {
            listener_port: listener_port.clone(),
            proof_of_work_stamp: proof_of_work_stamp.clone(),
            public_key: public_key.clone(),
            secret_key: secret_key.clone()
        };
        let props = Props::new_args(Peer::actor, (event_channel, address.clone(), Arc::new(info)));
        let actor_id = ACTOR_ID_GENERATOR.fetch_add(1, Ordering::SeqCst);
        sys.actor_of(props, &format!("peer-{}", actor_id))
    }

    fn actor((event_channel, address, info): (ChannelRef<NetworkChannelMsg>, SocketAddr, Arc<Local>)) -> Self {
        Peer {
            event_channel,
            local: info,
            net: Network {
                rx_run: AtomicBool::new(false),
                tx: Arc::new(Mutex::new(None)),
            }
        }
    }
}

impl Actor for Peer {
    type Msg = PeerMsg;

    fn post_start(&mut self, ctx: &Context<Self::Msg>) {
        self.event_channel.tell(Publish { msg: PeerCreated { peer: ctx.myself() }.into(), topic: NetworkChannelTopic::NetworkEvents.into() }, None);
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        // Use the respective Receive<T> implementation
        self.receive(ctx, msg, sender);
    }
}

impl Receive<Bootstrap> for Peer {
    type Msg = PeerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Bootstrap, sender: Sender) {
        let info = self.local.clone();
        let myself = ctx.myself();
        let system = ctx.system.clone();
        let net = self.net.clone();
        let event_channel = self.event_channel.clone();

        ctx.run(async move {

            async fn set_net(net: &Network, tx: MessageSender) {
                net.rx_run.store(true, Ordering::Relaxed);
                *net.tx.lock().await = Some(tx);
            }

            match bootstrap(msg, info).await {
                Ok(BootstrapOutput(mut rx, tx)) => {
                    set_net(&net, tx).await;

                    event_channel.tell(Publish { msg: PeerBootstrapped.into(), topic: NetworkChannelTopic::NetworkEvents.into() }, Some(myself.clone().into()));

                    begin_process_incoming(rx, net.rx_run, myself, event_channel);
                }
                Err(e) => {
                    warn!("Connection to peer failed: {}", e);
                    system.stop(myself);
                }
            }
        });
    }
}

impl Receive<GetCurrentBranch> for Peer {
    type Msg = PeerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: GetCurrentBranch, sender: Sender) {
        let tx = self.net.tx.clone();
        ctx.run(async move {
            let get_current_branch: PeerMessageResponse = PeerMessage::GetCurrentBranch(GetCurrentBranchMessage::new(msg.chain_id.clone())).into();
            let tx = tx.lock().await.unwrap();
            match tx.write_message(&get_current_branch).await {
                Ok(()) => debug!("Write success"),
                Err(e) => warn!("Failed to write encoding. {:?}", e),
            }
        });
    }
}

impl Receive<SendMessage> for Peer {
    type Msg = PeerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: SendMessage, sender: Sender) {
        let tx = self.net.tx.clone();
        ctx.run(async move {
            let tx = tx.lock().await.unwrap();
            match tx.write_message(&msg.message).await {
                Ok(()) => debug!("Write success"),
                Err(e) => warn!("Failed to write encoding. {:?}", e),
            }
        });
    }
}

/// Output values of the successful bootstrap process
struct BootstrapOutput(MessageReceiver, MessageSender);

async fn bootstrap(msg: Bootstrap, info: Arc<Local>) -> Result<BootstrapOutput, PeerError> {
    let (mut msg_rx, mut msg_tx) = {
        let stream = msg.stream.lock().await.take().expect("Someone took ownership of the socket before the Peer");
        let msg_reader: MessageStream = stream.into();
        msg_reader.split()
    };

    // send connection message
    let connection_message = ConnectionMessage::new(
        info.listener_port,
        &info.public_key,
        &info.proof_of_work_stamp,
        &Nonce::random().get_bytes(),
        vec![supported_version()]);
    let connection_message_sent = {
        let as_bytes = connection_message.as_bytes()?;
        match msg_tx.write_message(&as_bytes).await {
            Ok(bytes) => bytes,
            Err(e) => return Err(PeerError::NetworkError { error: e, message: "Failed to transfer connection message" })
        }
    };

    // receive connection message
    let received_connection_msg = match msg_rx.read_message().await {
        Ok(msg) => msg,
        Err(e) => return Err(PeerError::NetworkError { error: e, message: "Receive no response to our connection message" })
    };
    // generate local and remote nonce
    let NoncePair { local: nonce_local, remote: nonce_remote } = generate_nonces(&connection_message_sent, &received_connection_msg, msg.incoming);

    // convert received bytes from remote peer into `ConnectionMessage`
    let received_connection_msg: ConnectionMessage = ConnectionMessage::try_from(received_connection_msg)?;
    let peer_public_key = received_connection_msg.get_public_key();
    let peer_id = hex::encode(&peer_public_key);
    debug!("Received peer_public_key: {}", &peer_id);

    // pre-compute encryption key
    let precomputed_key = match precompute(&hex::encode(peer_public_key), &info.secret_key) {
        Ok(key) => key,
        Err(_) => return Err(PeerError::FailedToPrecomputeKey)
    };

    // from now on all messages will be encrypted
    let mut msg_tx = MessageSender {
        precomputed_key: precomputed_key.clone(),
        nonce_local,
        tx: msg_tx,
        peer_id: peer_id.clone(),
    };
    let mut msg_rx = MessageReceiver {
        precomputed_key,
        nonce_remote,
        rx: msg_rx,
        peer_id,
    };

    // send metadata
    let metadata = MetadataMessage::new(false, false);
    msg_tx.write_message(&metadata).await?;

    // receive metadata
    let metadata_received = MetadataMessage::from_bytes(msg_rx.read_message().await?)?;
    debug!("Received remote peer metadata - disable_mempool: {}, private_node: {}", metadata_received.disable_mempool, metadata_received.private_node);

    // send ack
    msg_tx.write_message(&AckMessage::Ack).await?;

    // receive ack
    let ack_received = AckMessage::from_bytes(msg_rx.read_message().await?)?;

    match ack_received {
        AckMessage::Ack => {
            debug!("Received remote peer ack/nack - ACK");
            Ok(BootstrapOutput(msg_rx, msg_tx))
        }
        AckMessage::Nack => {
            Err(PeerError::NackReceived)
        }
    }
}


/// Generate nonces (sent and recv encoding must be with length bytes also)
///
/// local_nonce is used for writing crypto messages to other peers
/// remote_nonce is used for reading crypto messages from other peers
fn generate_nonces(sent_msg: &RawBinaryMessage, recv_msg: &RawBinaryMessage, incoming: bool) -> NoncePair {
    nonce::generate_nonces(sent_msg.get_raw(), recv_msg.get_raw(), incoming)
}

/// Return supported network protocol version
fn supported_version() -> Version {
    Version::new("TEZOS_ALPHANET_2018-11-30T15:30:56Z".into(), 0, 0)
}

/// Start to process incoming data
async fn begin_process_incoming(mut rx: MessageReceiver, rx_run: AtomicBool, myself: PeerRef, event_channel: ChannelRef<NetworkChannelMsg>) {
    info!("Starting accepting messages from peer: {}", &rx.peer_id);

    while rx_run.load(Ordering::Relaxed) {
        if let Ok(msg) = rx.read_message().await {
            match PeerMessageResponse::from_bytes(msg) {
                Ok(msg) => {
                    info!("Message parsed successfully");
                    event_channel.tell(Publish { msg: PeerMessageReceived {
                        message: Arc::new(msg)
                    }.into(), topic: DEFAULT_TOPIC.into() }, Some(myself.clone().into()));
                }
                Err(e) => warn!("Failed to process received message: {:?}", e)
            }
        } else {
            // TODO: notify disconnect event / stop actor
        }
    }
}
