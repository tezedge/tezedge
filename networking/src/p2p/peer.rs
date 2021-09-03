// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::lock::Mutex;
use riker::actors::*;
use slog::{debug, info, o, trace, warn, Logger};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::runtime::Handle;
use tokio::sync::Notify;
use tokio::time::timeout;

use crypto::{
    blake2b::Blake2bError,
    crypto_box::{CryptoKey, PrecomputedKey, PublicKey},
    proof_of_work::PowError,
};
use crypto::{
    crypto_box::PublicKeyError,
    hash::{CryptoboxPublicKeyHash, Hash},
};
use crypto::{
    nonce::{self, Nonce, NoncePair},
    proof_of_work::check_proof_of_work,
};
use tezos_encoding::{binary_reader::BinaryReaderError, binary_writer::BinaryWriterError};
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryChunkError, BinaryRead, BinaryWrite};
use tezos_messages::p2p::encoding::ack::{NackInfo, NackMotive};
use tezos_messages::p2p::encoding::prelude::*;

use crate::p2p::network_channel::NetworkChannelMsg;
use crate::p2p::peer::quota::get_reset_period;
use crate::{LocalPeerInfo, PeerId};

use self::quota::ThrottleQuota;

use super::network_channel::{NetworkChannelRef, NetworkChannelTopic, PeerMessageReceived};
use super::stream::{EncryptedMessageReader, EncryptedMessageWriter, MessageStream, StreamError};

const IO_TIMEOUT: Duration = Duration::from_secs(6);
/// There is a 90-second timeout for ping peers with GetCurrentHead
const READ_TIMEOUT_LONG: Duration = Duration::from_secs(120);

#[derive(Debug, Error)]
pub enum PeerError {
    #[error("Unsupported protocol - shell: ({supported_version}) is not compatible with peer: ({incompatible_version})")]
    UnsupportedProtocol {
        supported_version: String,
        incompatible_version: String,
    },
    #[error("Received NACK from remote peer")]
    NackReceived,
    #[error("Received NACK from remote peer with info: {nack_info:?}")]
    NackWithMotiveReceived { nack_info: NackInfo },
    #[error("Network error: {message}, reason: {error}")]
    NetworkError {
        error: anyhow::Error,
        message: &'static str,
    },
    #[error("Message serialization error, reason: {error}")]
    SerializationError { error: BinaryWriterError },
    #[error("Message deserialization error, reason: {error}")]
    DeserializationError { error: BinaryReaderError },
    #[error("Crypto error, reason: {error}")]
    CryptoError { error: crypto::CryptoError },
    #[error("Public key error: {0}")]
    PublicKeyError(PublicKeyError),
    #[error("Not enough proof of work: {0}")]
    PowError(PowError),
}

impl From<BinaryWriterError> for PeerError {
    fn from(error: BinaryWriterError) -> Self {
        PeerError::SerializationError { error }
    }
}

impl From<BinaryReaderError> for PeerError {
    fn from(error: BinaryReaderError) -> Self {
        PeerError::DeserializationError { error }
    }
}

impl From<std::io::Error> for PeerError {
    fn from(error: std::io::Error) -> Self {
        PeerError::NetworkError {
            error: error.into(),
            message: "Network error",
        }
    }
}

impl From<StreamError> for PeerError {
    fn from(error: StreamError) -> Self {
        PeerError::NetworkError {
            error: error.into(),
            message: "Stream error",
        }
    }
}

impl From<BinaryChunkError> for PeerError {
    fn from(error: BinaryChunkError) -> Self {
        PeerError::NetworkError {
            error: error.into(),
            message: "Binary chunk error",
        }
    }
}

impl From<crypto::CryptoError> for PeerError {
    fn from(error: crypto::CryptoError) -> Self {
        PeerError::CryptoError { error }
    }
}

impl From<tokio::time::error::Elapsed> for PeerError {
    fn from(timeout: tokio::time::error::Elapsed) -> Self {
        PeerError::NetworkError {
            message: "Connection timeout",
            error: timeout.into(),
        }
    }
}

impl From<PublicKeyError> for PeerError {
    fn from(source: PublicKeyError) -> Self {
        PeerError::PublicKeyError(source)
    }
}

impl From<Blake2bError> for PeerError {
    fn from(source: Blake2bError) -> Self {
        PeerError::PublicKeyError(source.into())
    }
}

/// Commands peer actor to initialize bootstrapping process with a remote peer.
#[derive(Clone, Debug)]
pub struct Bootstrap {
    stream: Arc<Mutex<Option<TcpStream>>>,
    address: SocketAddr,
    incoming: bool,
    disable_mempool: bool,
    private_node: bool,
}

impl Bootstrap {
    pub fn incoming(
        stream: Arc<Mutex<Option<TcpStream>>>,
        address: SocketAddr,
        disable_mempool: bool,
        private_node: bool,
    ) -> Self {
        Bootstrap {
            stream,
            address,
            incoming: true,
            disable_mempool,
            private_node,
        }
    }

    pub fn outgoing(
        stream: TcpStream,
        address: SocketAddr,
        disable_mempool: bool,
        private_node: bool,
    ) -> Self {
        Bootstrap {
            stream: Arc::new(Mutex::new(Some(stream))),
            address,
            incoming: false,
            disable_mempool,
            private_node,
        }
    }
}

/// Commands peer actor to send a p2p message to a remote peer.
#[derive(Clone, Debug)]
pub struct SendMessage {
    /// Message is wrapped in `Arc` to avoid excessive cloning.
    message: Arc<PeerMessageResponse>,
}

impl SendMessage {
    pub fn new(message: Arc<PeerMessageResponse>) -> Self {
        SendMessage { message }
    }
}

#[derive(Clone)]
struct Network {
    /// Message receiver boolean indicating whether
    /// more messages should be received from network
    rx_run: Arc<AtomicBool>,
    /// Message sender
    tx: Arc<Mutex<Option<EncryptedMessageWriter>>>,
    /// Message receiver
    rx: Arc<Mutex<Option<EncryptedMessageReader>>>,
    /// Socket address of the peer
    socket_address: SocketAddr,
}

mod quota;

pub type PeerRef = ActorRef<PeerMsg>;

/// Represents a single p2p peer.
#[actor(SendMessage)]
pub struct Peer {
    /// All events generated by the peer will end up in this channel
    network_channel: NetworkChannelRef,
    /// Network IO
    net: Network,
    /// Tokio task executor
    tokio_executor: Handle,
    /// bootstrap output
    peer_public_key_hash: CryptoboxPublicKeyHash,
    peer_id_marker: String,
    peer_metadata: MetadataMessage,
    peer_compatible_network_version: NetworkVersion,
    throttle_quota: Arc<std::sync::Mutex<quota::ThrottleQuota>>,
    quota_update_stop: Arc<Notify>,
}

impl Peer {
    /// Create instance of a peer actor.
    pub fn actor(
        peer_actor_name: &str,
        sys: &impl ActorRefFactory,
        network_channel: NetworkChannelRef,
        tokio_executor: Handle,
        info: BootstrapOutput,
        log: &Logger,
    ) -> Result<PeerRef, CreateError> {
        sys.actor_of_props(
            peer_actor_name,
            Props::new_args::<Peer, _>((
                network_channel,
                tokio_executor,
                info,
                log.new(o!("peer_uri" => peer_actor_name.to_string())),
            )),
        )
    }
}

impl ActorFactoryArgs<(NetworkChannelRef, Handle, BootstrapOutput, Logger)> for Peer {
    fn create_args(
        (event_channel, tokio_executor, info, log): (
            NetworkChannelRef,
            Handle,
            BootstrapOutput,
            Logger,
        ),
    ) -> Self {
        Peer {
            network_channel: event_channel,
            net: Network {
                rx_run: Arc::new(AtomicBool::new(false)),
                tx: info.1,
                rx: info.0,
                socket_address: info.6,
            },
            tokio_executor,
            peer_public_key_hash: info.2,
            peer_id_marker: info.3,
            peer_metadata: info.4,
            peer_compatible_network_version: info.5,
            throttle_quota: Arc::new(std::sync::Mutex::new(ThrottleQuota::new(log))),
            quota_update_stop: Arc::new(Notify::new()),
        }
    }
}

impl Actor for Peer {
    type Msg = PeerMsg;

    fn post_stop(&mut self) {
        self.net.rx_run.store(false, Ordering::Release);
        self.quota_update_stop.notify_one();
    }

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // quota replenish task

        let throttle_quota = self.throttle_quota.clone();
        let log = ctx.system.log().clone();
        let stop = self.quota_update_stop.clone();
        self.tokio_executor.spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(get_reset_period())) => {
                        match throttle_quota.lock() {
                            Ok(ref mut quota) => quota.reset_all(),
                            Err(e) => warn!(log, "Failed to obtain a lock on throttling quota"; "reason" => format!("{:?}", e)),
                        }
                    }
                    _ = stop.notified() => {
                        return;
                    }
                }
            }
        });

        let myself = ctx.myself();
        let system = ctx.system.clone();
        let net = self.net.clone();
        let network_channel = self.network_channel.clone();
        let peer_public_key_hash = self.peer_public_key_hash.clone();
        let peer_id_marker = self.peer_id_marker.clone();
        let peer_metadata = self.peer_metadata.clone();
        let peer_compatible_network_version = self.peer_compatible_network_version.clone();
        let throttle_quota = self.throttle_quota.clone();

        self.tokio_executor.spawn(async move {
            // prepare PeerId
            let peer_id = Arc::new(PeerId::new(myself.clone(), peer_public_key_hash, peer_id_marker, net.socket_address));
            let log = {
                let myself_name = myself.name().to_string();
                let myself_uri = myself.uri().to_string();
                system.log().new(slog::o!("peer_id" => peer_id.peer_id_marker.clone(), "peer_ip" => net.socket_address.to_string(), "peer" => myself_name, "peer_uri" => myself_uri))
            };
            debug!(log, "Bootstrap successful"; "peer_metadata" => format!("{:?}", &peer_metadata));

            // setup encryption writer
            net.rx_run.store(true, Ordering::Release);

            // Network event - notify that peer was bootstrapped successfully
            network_channel.tell(Publish {
                msg: NetworkChannelMsg::PeerBootstrapped(peer_id.clone(), Arc::new(peer_metadata), Arc::new(peer_compatible_network_version)),
                topic: NetworkChannelTopic::NetworkEvents.into(),
            }, None);

            // begin to process incoming messages in a loop
            begin_process_incoming(net, myself.clone(), network_channel, throttle_quota, log.clone()).await;

            // connection to peer was closed, stop this actor
            system.stop(myself);
        });
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        // Use the respective Receive<T> implementation
        self.receive(ctx, msg, sender);
    }
}

impl Receive<SendMessage> for Peer {
    type Msg = PeerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: SendMessage, _sender: Sender) {
        match self.throttle_quota.lock() {
            Ok(ref mut quota) => {
                if !quota.can_send(msg.message.as_ref()) {
                    return;
                }
            }
            Err(e) => warn!(
                ctx.system.log(),
                "Failed to obtain a lock on throttling quota";
                "reason" => format!("{:?}", e)
            ),
        }

        let system = ctx.system.clone();
        let myself = ctx.myself();
        let tx = self.net.tx.clone();
        let peer_id_marker = self.peer_id_marker.clone();

        self.tokio_executor.spawn(async move {
            let mut tx_lock = tx.lock().await;
            if let Some(tx) = tx_lock.as_mut() {
                let write_result =
                    timeout(IO_TIMEOUT, tx.write_message(msg.message.as_ref())).await;
                // release mutex as soon as possible
                drop(tx_lock);

                match write_result {
                    Ok(write_result) => {
                        if let Err(e) = write_result {
                            warn!(system.log(), "Failed to send message"; "reason" => e, "msg" => format!("{:?}", msg.message.as_ref()),
                                                "peer_id" => peer_id_marker, "peer" => myself.name(), "peer_uri" => myself.uri().to_string());
                            system.stop(myself);
                        }
                    }
                    Err(_) => {
                        warn!(system.log(), "Failed to send message"; "reason" => "timeout", "msg" => format!("{:?}", msg.message.as_ref()),
                                            "peer_id" => peer_id_marker, "peer" => myself.name(), "peer_uri" => myself.uri().to_string());
                        system.stop(myself);
                    }
                }
            }
        });
    }
}

/// Output values of the successful bootstrap process
#[derive(Clone)]
pub struct BootstrapOutput(
    pub Arc<Mutex<Option<EncryptedMessageReader>>>,
    pub Arc<Mutex<Option<EncryptedMessageWriter>>>,
    pub CryptoboxPublicKeyHash,
    pub String,
    pub MetadataMessage,
    pub NetworkVersion,
    pub SocketAddr,
);

impl fmt::Debug for BootstrapOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let BootstrapOutput(
            _,
            _,
            peer_public_key_hash,
            peer_id_marker,
            peer_metadata,
            peer_compatible_network_version,
            peer_address,
        ) = self;
        let peer_public_key_hash: &Hash = peer_public_key_hash.as_ref();
        f.debug_tuple("BootstrapOutput")
            .field(&hex::encode(peer_public_key_hash))
            .field(peer_id_marker)
            .field(peer_metadata)
            .field(peer_compatible_network_version)
            .field(peer_address)
            .finish()
    }
}

pub async fn bootstrap(
    msg: Bootstrap,
    info: Arc<LocalPeerInfo>,
    log: &Logger,
) -> Result<BootstrapOutput, PeerError> {
    let (mut msg_rx, mut msg_tx) = {
        let stream = msg
            .stream
            .lock()
            .await
            .take()
            .expect("Someone took ownership of the socket before the Peer");
        let msg_reader: MessageStream = stream.into();
        msg_reader.split()
    };

    let supported_protocol_version = &info.version;

    // send connection message
    let connection_message = ConnectionMessage::try_new(
        info.listener_port,
        &info.identity.public_key,
        &info.identity.proof_of_work_stamp,
        Nonce::random(),
        supported_protocol_version.as_ref().to_network_version(),
    )?;
    let connection_message_sent = {
        let connection_message_bytes = BinaryChunk::from_content(&connection_message.as_bytes()?)?;
        match timeout(IO_TIMEOUT, msg_tx.write_message(&connection_message_bytes)).await? {
            Ok(_) => connection_message_bytes,
            Err(e) => {
                return Err(PeerError::NetworkError {
                    error: e.into(),
                    message: "Failed to transfer connection message",
                })
            }
        }
    };

    // receive connection message
    let received_connection_message_bytes = match timeout(IO_TIMEOUT, msg_rx.read_message()).await?
    {
        Ok(msg) => msg,
        Err(e) => {
            return Err(PeerError::NetworkError {
                error: e.into(),
                message: "No response to connection message was received",
            })
        }
    };

    let connection_message =
        ConnectionMessage::from_bytes(received_connection_message_bytes.content())?;

    // create PublicKey from received bytes from remote peer
    let peer_public_key = PublicKey::from_bytes(connection_message.public_key())?;

    let connecting_to_self = peer_public_key == info.identity.public_key;
    if connecting_to_self {
        warn!(log, "Detected self connection");
        // treat as if nack was received
        return Err(PeerError::NackWithMotiveReceived {
            nack_info: NackInfo::new(NackMotive::AlreadyConnected, &[]),
        });
    }

    // make sure the peer performed enough crypto calculations
    if let Err(e) = check_proof_of_work(
        &received_connection_message_bytes.raw()[4..60],
        info.pow_target,
    ) {
        return Err(PeerError::PowError(e));
    }

    // generate local and remote nonce
    let NoncePair {
        local: nonce_local,
        remote: nonce_remote,
    } = generate_nonces(
        &connection_message_sent,
        &received_connection_message_bytes,
        msg.incoming,
    )?;

    // pre-compute encryption key
    let precomputed_key = PrecomputedKey::precompute(&peer_public_key, &info.identity.secret_key);

    // generate public key hash for PublicKey, which will be used as a peer_id
    let peer_public_key_hash = peer_public_key.public_key_hash()?;
    let peer_id_marker = peer_public_key_hash.to_base58_check();
    let log = log.new(o!("peer_id" => peer_id_marker.clone()));

    // from now on all messages will be encrypted
    let mut msg_rx =
        EncryptedMessageReader::new(msg_rx, precomputed_key.clone(), nonce_remote, log.clone());
    let mut msg_tx = EncryptedMessageWriter::new(msg_tx, precomputed_key, nonce_local, log.clone());

    // send metadata
    let metadata = MetadataMessage::new(msg.disable_mempool, msg.private_node);
    timeout(IO_TIMEOUT, msg_tx.write_message(&metadata)).await??;

    // receive metadata
    let metadata_received = timeout(IO_TIMEOUT, msg_rx.read_message::<MetadataMessage>()).await??;
    debug!(log, "Received remote peer metadata";
                "disable_mempool" => metadata_received.disable_mempool(),
                "private_node" => metadata_received.private_node(),
                "port" => connection_message.port(),
    );

    let peer_version = connection_message.version();

    let compatible_network_version =
        match supported_protocol_version.choose_compatible_version(peer_version) {
            Ok(compatible_version) => compatible_version,
            Err(nack_motive) => {
                // send nack
                if peer_version.supports_nack_with_list_and_motive() {
                    timeout(
                        IO_TIMEOUT,
                        msg_tx.write_message(&AckMessage::Nack(NackInfo::new(nack_motive, &[]))),
                    )
                    .await??;
                } else {
                    timeout(IO_TIMEOUT, msg_tx.write_message(&AckMessage::NackV0)).await??;
                }

                return Err(PeerError::UnsupportedProtocol {
                    supported_version: format!(
                        "{}/distributed_db_versions {:?}/p2p_versions {:?}",
                        supported_protocol_version.version.chain_name(),
                        supported_protocol_version.distributed_db_versions,
                        supported_protocol_version.p2p_versions
                    ),
                    incompatible_version: format!(
                        "{}/distributed_db_version {}/p2p_version {}",
                        peer_version.chain_name(),
                        peer_version.distributed_db_version(),
                        peer_version.p2p_version()
                    ),
                });
            }
        };

    // send ack
    timeout(IO_TIMEOUT, msg_tx.write_message(&AckMessage::Ack)).await??;

    // receive ack
    let ack_received = timeout(IO_TIMEOUT, msg_rx.read_message()).await??;

    match ack_received {
        AckMessage::Ack => {
            debug!(log, "Received ACK");
            Ok(BootstrapOutput(
                Arc::new(Mutex::new(Some(msg_rx))),
                Arc::new(Mutex::new(Some(msg_tx))),
                peer_public_key_hash,
                peer_id_marker,
                metadata_received,
                compatible_network_version,
                msg.address,
            ))
        }
        AckMessage::NackV0 => {
            debug!(log, "Received NACK");
            Err(PeerError::NackReceived)
        }
        AckMessage::Nack(nack_info) => {
            debug!(log, "Received NACK with info: {:?}", nack_info);
            Err(PeerError::NackWithMotiveReceived { nack_info })
        }
    }
}

/// Generate nonces (sent and recv encoding must be with length bytes also)
///
/// local_nonce is used for writing crypto messages to other peers
/// remote_nonce is used for reading crypto messages from other peers
fn generate_nonces(
    sent_msg: &BinaryChunk,
    recv_msg: &BinaryChunk,
    incoming: bool,
) -> Result<NoncePair, Blake2bError> {
    nonce::generate_nonces(sent_msg.raw(), recv_msg.raw(), incoming)
}

/// Start to process incoming data
async fn begin_process_incoming(
    net: Network,
    myself: PeerRef,
    event_channel: NetworkChannelRef,
    throttle_quota: Arc<std::sync::Mutex<ThrottleQuota>>,
    log: Logger,
) {
    info!(log, "Starting to accept messages");

    let mut rx = net.rx.lock().await;
    let mut rx = rx
        .take()
        .expect("Someone took ownership of the encrypted reader before the Peer");
    while net.rx_run.load(Ordering::Acquire) {
        match timeout(READ_TIMEOUT_LONG, rx.read_message::<PeerMessageResponse>()).await {
            Ok(res) => match res {
                Ok(msg) => match throttle_quota.lock() {
                    Ok(ref mut quota) => {
                        if quota.can_receive(&msg) {
                            let should_broadcast_message = net.rx_run.load(Ordering::Acquire);
                            if should_broadcast_message {
                                trace!(log, "Message parsed successfully"; "msg" => format!("{:?}", &msg));
                                event_channel.tell(
                                    Publish {
                                        msg: PeerMessageReceived {
                                            peer: myself.clone(),
                                            message: Arc::new(msg),
                                        }
                                        .into(),
                                        topic: NetworkChannelTopic::NetworkEvents.into(),
                                    },
                                    None,
                                );
                            }
                        }
                    }
                    Err(e) => {
                        warn!(log, "Failed to obtain a lock for throttle quota"; "reason" => format!("{:?}", e))
                    }
                },
                Err(StreamError::DeserializationError { error }) => match error {
                    BinaryReaderError::UnknownTag(tag) => {
                        warn!(log, "Messages with unsupported tags are ignored"; "tag" => tag);
                    }
                    error => {
                        warn!(log, "Failed to read peer message"; "reason" => StreamError::DeserializationError{ error });
                        break;
                    }
                },
                Err(e) => {
                    warn!(log, "Failed to read peer message"; "reason" => e);
                    break;
                }
            },
            Err(_) => {
                warn!(log, "Peer message read timed out"; "secs" => READ_TIMEOUT_LONG.as_secs());
                break;
            }
        }
    }

    debug!(log, "Shutting down peer connection");
    let mut tx_lock = net.tx.lock().await;
    if let Some(tx) = tx_lock.take() {
        let mut socket = rx.unsplit(tx);
        match socket.shutdown().await {
            Ok(()) => {
                debug!(log, "Connection shutdown successful"; "socket" => format!("{:?}", socket))
            }
            Err(err) => {
                debug!(log, "Failed to shutdown connection"; "err" => format!("{:?}", err), "socket" => format!("{:?}", socket))
            }
        }
    }

    info!(log, "Stopped to accept messages");
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        sync::{
            atomic::{AtomicIsize, AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };

    use crypto::hash::CryptoboxPublicKeyHash;
    use futures::lock::Mutex;
    use riker::{
        actor::ActorRefFactory,
        actors::{ActorSystem, SystemBuilder, Tell},
    };
    use slog::{Drain, Level, Logger, KV};
    use tezos_identity::Identity;
    use tezos_messages::p2p::encoding::{
        metadata::MetadataMessage,
        peer::{PeerMessage, PeerMessageResponse},
        prelude::AdvertiseMessage,
        version::NetworkVersion,
    };
    use tokio::runtime::Handle;

    use crate::p2p::{
        network_channel::{NetworkChannel, NetworkChannelRef},
        peer::ThrottleQuota,
    };

    use super::{BootstrapOutput, Peer, PeerRef, SendMessage};

    fn create_logger(warns: Arc<AtomicUsize>, exceeded: Arc<AtomicIsize>, level: Level) -> Logger {
        let drain = slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
            .build()
            .fuse();

        struct MySer(Arc<AtomicIsize>);

        impl slog::Serializer for MySer {
            fn emit_arguments(
                &mut self,
                key: slog::Key,
                val: &core::fmt::Arguments,
            ) -> slog::Result {
                if key == "amount" {
                    self.0
                        .store(val.to_string().parse().unwrap(), Ordering::Relaxed);
                }
                Ok(())
            }
        }

        struct MyDrain {
            warns: Arc<AtomicUsize>,
            ser: RefCell<MySer>,
        }

        impl Drain for MyDrain {
            type Ok = ();
            type Err = slog::Never;

            fn log(
                &self,
                record: &slog::Record,
                _values: &slog::OwnedKVList,
            ) -> std::result::Result<Self::Ok, Self::Err> {
                if record.level() == Level::Warning {
                    let msg = record.msg().to_string();
                    if msg == "Cannot send message because its send quota is exceeded" {
                        self.warns.fetch_add(1, Ordering::Relaxed);
                    } else if msg == String::from("Tx quota is exceeded") {
                        record
                            .kv()
                            .serialize(record, &mut *self.ser.borrow_mut())
                            .unwrap();
                    }
                }
                Ok(())
            }
        }

        let drain = slog_async::Async::new(
            slog::Duplicate::new(
                MyDrain {
                    warns,
                    ser: RefCell::new(MySer(exceeded)),
                },
                drain,
            )
            .fuse(),
        )
        .build()
        .filter_level(level)
        .fuse();

        Logger::root(drain, slog::o!())
    }

    fn create_test_actor_system(log: Logger) -> ActorSystem {
        SystemBuilder::new()
            .name("create_actor_system")
            .log(log)
            .create()
            .expect("Failed to create test actor system")
    }

    fn create_test_tokio_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create test tokio runtime")
    }

    fn create_test_peer(
        sys: &impl ActorRefFactory,
        network_channel: NetworkChannelRef,
        tokio_executor: Handle,
        log: Logger,
    ) -> PeerRef {
        let node_identity = Arc::new(Identity::generate(0f64).unwrap());
        let peer_public_key_hash: CryptoboxPublicKeyHash =
            node_identity.public_key.public_key_hash().unwrap();
        let peer_id_marker = peer_public_key_hash.to_base58_check();

        Peer::actor(
            "test-peer",
            sys,
            network_channel,
            tokio_executor,
            BootstrapOutput(
                Arc::new(Mutex::new(None)),
                Arc::new(Mutex::new(None)),
                peer_public_key_hash,
                peer_id_marker,
                MetadataMessage::new(false, false).clone(),
                NetworkVersion::new("".to_owned(), 0, 0),
                "127.0.0.1:9732".parse().unwrap(),
            ),
            &log,
        )
        .expect("Cannot create a test actor")
    }

    fn create_test_mgs() -> PeerMessageResponse {
        PeerMessage::Advertise(AdvertiseMessage::new(&[])).into()
    }

    #[test]
    #[ignore]
    fn test_quota_exceeded() {
        let received_messages = Arc::new(AtomicUsize::new(0));
        let quota_exceeded = Arc::new(AtomicIsize::new(0));
        let log = create_logger(
            received_messages.clone(),
            quota_exceeded.clone(),
            Level::Debug,
        );
        let actor_system = create_test_actor_system(log.clone());
        let runtime = create_test_tokio_runtime();
        let network_channel =
            NetworkChannel::actor(&actor_system).expect("Failed to create network channel");
        let peer = create_test_peer(
            &actor_system,
            network_channel.clone(),
            runtime.handle().clone(),
            log.clone(),
        );
        let msg = create_test_mgs();
        let idx = ThrottleQuota::msg_index(&msg);
        (0..super::quota::THROTTLING_QUOTA_MAX[idx].0 + 11).for_each(|_| {
            peer.tell(SendMessage::new(Arc::new(msg.clone())), None);
        });
        std::thread::sleep(Duration::from_millis(50));

        // Only a single warning on dropped messages
        assert_eq!(received_messages.load(Ordering::Relaxed), 1);

        std::thread::sleep(Duration::from_millis(
            *super::quota::THROTTLING_QUOTA_RESET_MS,
        ));

        // Quota is exceeded by 11
        assert_eq!(
            quota_exceeded.load(Ordering::Relaxed) - super::quota::THROTTLING_QUOTA_MAX[idx].0,
            11
        );

        for _ in 0..super::quota::THROTTLING_QUOTA_MAX[idx].0 + 10 {
            peer.tell(SendMessage::new(Arc::new(msg.clone())), None);
        }
        std::thread::sleep(Duration::from_millis(50));

        // One more warning on dropped messages
        assert_eq!(received_messages.load(Ordering::Relaxed), 2);

        std::thread::sleep(Duration::from_millis(
            *super::quota::THROTTLING_QUOTA_RESET_MS,
        ));

        // Quota is exceeded by 11
        assert_eq!(
            quota_exceeded.load(Ordering::Relaxed) - super::quota::THROTTLING_QUOTA_MAX[idx].0,
            10
        );
    }
}
