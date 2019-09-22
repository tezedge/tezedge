use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use futures::lock::Mutex;
use log::info;
use riker::actors::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::TaskExecutor;

use super::network_channel::NetworkChannelMsg;
use super::peer::{Bootstrap, Peer, PeerRef};
use std::sync::atomic::{Ordering, AtomicBool};

/// Blacklist the IP address.
#[derive(Clone, Debug)]
pub struct BlacklistIpAddress {
    address: IpAddr,
}

/// Whitelist the IP address.
#[derive(Clone, Debug)]
pub struct WhitelistIpAddress {
    address: IpAddr,
}

/// Accept incoming peer connection.
#[derive(Clone, Debug)]
pub struct AcceptPeer {
    stream: Arc<Mutex<Option<TcpStream>>>,
    address: SocketAddr,
}

/// Open connection to the remote peer node.
#[derive(Clone, Debug)]
pub struct ConnectToPeer {
    pub address: SocketAddr
}

pub type NetworkManagerRef = ActorRef<NetworkManagerMsg>;

#[actor(BlacklistIpAddress, WhitelistIpAddress, AcceptPeer, ConnectToPeer)]
pub struct NetworkManager {
    event_channel: ChannelRef<NetworkChannelMsg>,
    tokio_executor: TaskExecutor,
    listener_port: u16,
    public_key: String,
    secret_key: String,
    proof_of_work_stamp: String,
    /// Message receiver boolean indicating whether
    /// more connections should be accepted from network
    rx_run: Arc<AtomicBool>,
}

impl NetworkManager {
    pub fn actor(sys: &impl ActorRefFactory,
                 event_channel: ChannelRef<NetworkChannelMsg>,
                 tokio_executor: TaskExecutor,
                 listener_port: u16,
                 public_key: String,
                 secret_key: String,
                 proof_of_work_stamp: String) -> Result<NetworkManagerRef, CreateError>
    {
        sys.actor_of(
            Props::new_args(NetworkManager::new, (event_channel, tokio_executor, listener_port, public_key, secret_key, proof_of_work_stamp)),
            NetworkManager::name())
    }

    /// The `NetworkManager` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    fn name() -> &'static str {
        "network-manager"
    }

    fn new((event_channel, tokio_executor, listener_port, public_key, secret_key, proof_of_work_stamp): (ChannelRef<NetworkChannelMsg>, TaskExecutor, u16, String, String, String)) -> Self {
        NetworkManager {
            event_channel,
            tokio_executor,
            listener_port,
            public_key,
            secret_key,
            proof_of_work_stamp,
            rx_run: Arc::new(AtomicBool::new(true)),
        }
    }

    fn create_peer(&self, sys: &impl ActorRefFactory) -> PeerRef {
        Peer::new(
            sys,
            self.event_channel.clone(),
            self.listener_port,
            &self.public_key,
            &self.secret_key,
            &self.proof_of_work_stamp,
            self.tokio_executor.clone(),
        ).unwrap()
    }
}

impl Actor for NetworkManager {
    type Msg = NetworkManagerMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let listener_port = self.listener_port;
        let myself = ctx.myself();
        let rx_run = self.rx_run.clone();

        self.tokio_executor.spawn(async move {
            begin_listen_incoming(listener_port, myself, rx_run).await;
        });
    }

    fn post_stop(&mut self) {
        self.rx_run.store(false, Ordering::Relaxed);
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        // Use the respective Receive<T> implementation
        self.receive(ctx, msg, sender);
    }
}

impl Receive<BlacklistIpAddress> for NetworkManager {
    type Msg = NetworkManagerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: BlacklistIpAddress, _sender: Sender) {
        unimplemented!()
    }
}

impl Receive<WhitelistIpAddress> for NetworkManager {
    type Msg = NetworkManagerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: WhitelistIpAddress, _sender: Sender) {
        unimplemented!()
    }
}

impl Receive<ConnectToPeer> for NetworkManager {
    type Msg = NetworkManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ConnectToPeer, _sender: Sender) {
        let peer = self.create_peer(ctx);
        let system = ctx.system.clone();

        self.tokio_executor.spawn(async move {
            info!("Connecting to {}", &msg.address);
            match TcpStream::connect(&msg.address).await {
                Ok(stream) => {
                    info!("Connection to {} successful", &msg.address);
                    peer.tell(Bootstrap::outgoing(stream, msg.address), None);
                }
                Err(_) => {
                    info!("Connection to {} failed", &msg.address);
                    system.stop(peer);
                }
            }
        });
    }
}

impl Receive<AcceptPeer> for NetworkManager {
    type Msg = NetworkManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: AcceptPeer, _sender: Sender) {
        let peer = self.create_peer(ctx);
        peer.tell(Bootstrap::incoming(msg.stream, msg.address), None);
    }
}


/// Start to listen for incoming connections indefinitely.
async fn begin_listen_incoming(listener_port: u16, connection_manager: NetworkManagerRef, rx_run: Arc<AtomicBool>) {
    let listener_address = format!("127.0.0.1:{}", listener_port).parse::<SocketAddr>().unwrap();
    let mut listener = TcpListener::bind(&listener_address).await.unwrap();

    while rx_run.load(Ordering::Relaxed) {
        let (stream, address) = listener.accept().await.unwrap();
        connection_manager.tell(AcceptPeer { stream: Arc::new(Mutex::new(Some(stream))), address }, None);
    }
}