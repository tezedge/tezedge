use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use futures::lock::Mutex;
use log::info;
use riker::actors::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

use crate::p2p::peer::PeerRef;

use super::network_channel::{DEFAULT_TOPIC, NetworkChannelMsg};
use super::network_channel::PeerCreated;
use super::peer::{Bootstrap, Peer};

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
    stream: Arc<Mutex<TcpStream>>,
    address: SocketAddr,
}

/// Open connection to the remote peer node.
#[derive(Clone, Debug)]
pub struct ConnectToPeer {
    pub address: SocketAddr
}

/// Lookup for remote peers via DNS query
#[derive(Clone, Debug)]
pub struct DnsLookup {
    /// List of bootstrap DNS host names
    urls: Vec<String>
}

pub type NetworkManagerRef = ActorRef<NetworkManagerMsg>;

#[actor(BlacklistIpAddress, WhitelistIpAddress, AcceptPeer, ConnectToPeer)]
pub struct NetworkManager {
    event_channel: ChannelRef<NetworkChannelMsg>,
    listener_port: u16,
    public_key: String,
    secret_key: String,
    proof_of_work_stamp: String,
}

impl NetworkManager {

    pub fn new(sys: &ActorSystem,
               event_channel: ChannelRef<NetworkChannelMsg>,
               listener_port: u16,
               public_key: String,
               secret_key: String,
               proof_of_work_stamp: String) -> Result<NetworkManagerRef, CreateError>
    {
        sys.actor_of(
            Props::new_args(NetworkManager::actor, (event_channel, listener_port, public_key, secret_key, proof_of_work_stamp)),
            NetworkManager::name())
    }

    /// The `NetworkManager` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    fn name() -> &'static str {
        "network-manager"
    }

    fn actor((event_channel, listener_port, public_key, secret_key, proof_of_work_stamp): (ChannelRef<NetworkChannelMsg>, u16, String, String, String)) -> Self {
        NetworkManager { event_channel, listener_port, public_key, secret_key, proof_of_work_stamp }
    }

    fn create_peer(&self, sys: &ActorSystem, address: &SocketAddr) -> PeerRef {
        Peer::new(
            sys,
            self.event_channel.clone(),
            &address,
            self.listener_port,
            &self.public_key,
            &self.secret_key,
            &self.proof_of_work_stamp
        ).unwrap()
    }
}

impl Actor for NetworkManager {
    type Msg = NetworkManagerMsg;

    fn post_start(&mut self, ctx: &Context<Self::Msg>) {
        ctx.run(begin_listen_incoming(self.listener_port, ctx.myself()));
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
        let peer= self.create_peer(&ctx.system, &msg.address);
        let system = ctx.system.clone();

        ctx.run(async move {
            match TcpStream::connect(&msg.address).await {
                Ok(stream) => {
                    peer.tell(Bootstrap::outgoing(stream, msg.address), None);
                }
                Err(e) => {
                    info!("Connection to {:?} failed", &msg.address);
                    system.stop(peer);
                }
            }
        });
    }
}

impl Receive<AcceptPeer> for NetworkManager {
    type Msg = NetworkManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: AcceptPeer, sender: Sender) {
        let peer= self.create_peer(&ctx.system, &msg.address);

        ctx.run(async move {
            peer.tell(Bootstrap::incoming(stream, msg.address), None);
        });
    }
}


/// Start to listen for incoming connections indefinitely.
async fn begin_listen_incoming(listener_port: u16, connection_manager: NetworkManagerRef) {
    let listener_address = format!("127.0.0.1:{}", listener_port).parse().unwrap();
    let mut listener = TcpListener::bind(&listener_address).unwrap();

    loop {
        let (stream, address) = listener.accept().await.unwrap();
        connection_manager.tell(AcceptPeer { stream: Arc::new(Mutex::new(stream)), address }, None);
    }
}