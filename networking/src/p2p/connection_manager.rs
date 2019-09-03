use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use log::info;
use futures::lock::Mutex;
use riker::actors::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

use super::network_channel::{NetworkChannelMsg, DEFAULT_TOPIC};
use super::peer::{Peer, Bootstrap};
use super::network_channel::PeerCreated;
use crate::p2p::peer::PeerRef;

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

type ConnectionManagerRef = ActorRef<ConnectionManagerMsg>;

#[actor(BlacklistIpAddress, WhitelistIpAddress, AcceptPeer, ConnectToPeer)]
pub struct ConnectionManager {
    event_channel: ChannelRef<NetworkChannelMsg>,
    listener_port: u16,
    public_key: String,
    secret_key: String,
    proof_of_work_stamp: String,
}

impl ConnectionManager {

    pub fn new(sys: &ActorSystem,
               event_channel: ChannelRef<NetworkChannelMsg>,
               listener_port: u16,
               public_key: String,
               secret_key: String,
               proof_of_work_stamp: String) -> Result<ActorRef<ConnectionManagerMsg>, CreateError> {
        sys.actor_of(
            Props::new_args(ConnectionManager::actor, (event_channel, listener_port, public_key, secret_key, proof_of_work_stamp)),
            ConnectionManager::name())
    }

    /// The `ConnectionManager` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    fn name() -> &'static str {
        "connection-manager"
    }

    fn actor((event_channel, listener_port, public_key, secret_key, proof_of_work_stamp): (ChannelRef<NetworkChannelMsg>, u16, String, String, String)) -> Self {
        ConnectionManager { event_channel, listener_port, public_key, secret_key, proof_of_work_stamp }
    }

    fn create_advertise_peer(&self, sys: &ActorSystem, address: &SocketAddr) -> PeerRef {
        let peer = Peer::new(
            sys,
            self.event_channel.clone(),
            &address,
            self.listener_port,
            &self.public_key,
            &self.secret_key,
            &self.proof_of_work_stamp
        ).unwrap();

        // let the world know the new peer is here
        self.event_channel.tell(Publish { msg: PeerCreated { peer: peer.clone() }.into(), topic: DEFAULT_TOPIC.into() }, None);

        peer
    }
}

impl Actor for ConnectionManager {
    type Msg = ConnectionManagerMsg;

    fn post_start(&mut self, ctx: &Context<Self::Msg>) {
        begin_listen_incoming(self.listener_port, ctx.myself())
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        // Use the respective Receive<T> implementation
        self.receive(ctx, msg, sender);
    }
}

impl Receive<BlacklistIpAddress> for ConnectionManager {
    type Msg = ConnectionManagerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: BlacklistIpAddress, _sender: Sender) {
        unimplemented!()
    }
}

impl Receive<WhitelistIpAddress> for ConnectionManager {
    type Msg = ConnectionManagerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: WhitelistIpAddress, _sender: Sender) {
        unimplemented!()
    }
}

impl Receive<ConnectToPeer> for ConnectionManager {
    type Msg = ConnectionManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ConnectToPeer, _sender: Sender) {
        let peer= self.create_advertise_peer(&ctx.system, &msg.address);
        let system = ctx.system.clone();

        tokio::spawn(async move {
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

impl Receive<AcceptPeer> for ConnectionManager {
    type Msg = ConnectionManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: AcceptPeer, sender: Sender) {
        let peer= self.create_advertise_peer(&ctx.system, &msg.address);

        tokio::spawn(async move {
            peer.tell(Bootstrap::incoming(stream, msg.address), None);
        });
    }
}


/// Start to listen for incoming connections indefinitely.
fn begin_listen_incoming(listener_port: u16, connection_manager: ConnectionManagerRef) {

    tokio::spawn(async move {
        let listener_address = format!("127.0.0.1:{}", listener_port).parse().unwrap();
        let mut listener = TcpListener::bind(&listener_address).unwrap();

        loop {
            let (stream, address) = listener.accept().await.unwrap();
            connection_manager.tell(AcceptPeer { stream: Arc::new(Mutex::new(stream)), address }, None);
        }
    });

}