use std::collections::HashMap;
use std::net::{IpAddr, Shutdown, SocketAddr};
use std::sync::Arc;

use futures::lock::Mutex;
use riker::actors::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

use crate::network_channel::NetworkChannelMsg;
use crate::p2p::stream::MessageStream;
use crate::peer::{Peer, Bootstrap};
use crate::network_channel::PeerCreated;

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

/// Handle TCP connection.
#[derive(Clone, Debug)]
pub struct HandleConnection {
    stream: Arc<Mutex<TcpStream>>,
    address: SocketAddr,
}

/// Open connection to remote peer node.
#[derive(Clone, Debug)]
pub struct ConnectPeer {
    address: SocketAddr,
}

#[actor(BlacklistIpAddress, WhitelistIpAddress, HandleConnection, ConnectPeer)]
pub struct ConnectionManager {
    event_channel: ChannelRef<NetworkChannelMsg>,
}

impl ConnectionManager {

    pub fn new(sys: &ActorSystem, net_chan: ChannelRef<NetworkChannelMsg>) -> Result<ActorRef<ConnectionManagerMsg>, CreateError> {
        sys.actor_of(ConnectionManager::props(net_chan), ConnectionManager::name())
    }

    fn props(net_chan: ChannelRef<NetworkChannelMsg>) -> BoxActorProd<ConnectionManager> {
        Props::new_args(ConnectionManager::actor, net_chan)
    }

    /// The `ConnectionManager` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    fn name() -> &'static str {
        "connection-manager"
    }

    fn actor(message_channel: ChannelRef<NetworkChannelMsg>) -> Self {
        ConnectionManager { event_channel: message_channel, }
    }
}

impl Actor for ConnectionManager {
    type Msg = ConnectionManagerMsg;

    fn post_start(&mut self, ctx: &Context<Self::Msg>) {
        listen_incoming(ctx.myself())
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        // Use the respective Receive<T> implementation
        self.receive(ctx, msg, sender);
    }
}

impl Receive<BlacklistIpAddress> for ConnectionManager {
    type Msg = ConnectionManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: BlacklistIpAddress, sender: Sender) {
        unimplemented!()
    }
}

impl Receive<WhitelistIpAddress> for ConnectionManager {
    type Msg = ConnectionManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: WhitelistIpAddress, sender: Sender) {
        unimplemented!()
    }
}

impl Receive<ConnectPeer> for ConnectionManager {
    type Msg = ConnectionManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ConnectPeer, sender: Sender) {
        let peer = Peer::new(&ctx.system, self.event_channel.clone()).unwrap();
        // let the world know the new peer is here
        self.event_channel.tell(Publish { msg: PeerCreated { peer: peer.clone() }.into(), topic: "_none".into() }, None);

        tokio::spawn(async move {
            match TcpStream::connect(&msg.address).await {
                Ok(stream) => {
                    let stream: MessageStream = stream.into();
                    let (msg_rx, msg_tx) = stream.split();

                    peer.tell(Bootstrap { rx: Arc::new(Mutex::new(msg_rx)), tx: Arc::new(Mutex::new(msg_tx)) }, None);
                }
                Err(e) => {
                    // TODO: implement peer termination
                    //peer.tell(Publish { msg: PeerCreated { peer }, topic: "_none".into() }, Some(ctx.myself()));
                }
            }
        });
    }
}

impl Receive<HandleConnection> for ConnectionManager {
    type Msg = ConnectionManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: HandleConnection, sender: Sender) {
        tokio::spawn(async move {
            let mut buf = [0; 1024];

            let mut socket = msg.stream.lock().await;



            // In a loop, read data from the socket and write the data back.
            loop {
                let n = match socket.read(&mut buf).await {
                    // socket closed
                    Ok(n) if n == 0 => return,
                    Ok(n) => n,
                    Err(e) => {
                        println!("failed to read from socket; err = {:?}", e);
                        return;
                    }
                };

                // Write the data back
                if let Err(e) = socket.write_all(&buf[0..n]).await {
                    println!("failed to write to socket; err = {:?}", e);
                    return;
                }
            }
        });
    }
}



fn listen_incoming(conn_mgr: ActorRef<ConnectionManagerMsg>) {

    tokio::spawn(async move {
        let listener_address = "127.0.0.1:4578".parse().unwrap();
        let mut listener = TcpListener::bind(&listener_address).unwrap();

        loop {
            let (stream, address) = listener.accept().await.unwrap();

            conn_mgr.tell(HandleConnection { stream: Arc::new(Mutex::new(stream)), address }, None);
        }
    });

}