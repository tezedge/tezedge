use std::collections::HashMap;
use std::net::{SocketAddr, Shutdown};

use riker::actors::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct HandleIpConnection {
    address: SocketAddr,
    stream: Arc<Mutex<TcpStream>>,
}

#[derive(Clone, Debug)]
pub struct IpConnected {
    address: SocketAddr,
}

#[derive(Clone, Debug)]
pub struct BootstrapIp {
    address: SocketAddr
}

#[derive(Clone, Debug)]
pub struct DisconnectIp {
    address: SocketAddr
}


#[actor(HandleIpConnection, BootstrapIp, DisconnectIp)]
pub struct P2PServer {
    message_channel: ChannelRef<P2PServerMsg>,
    inbound_connections: HashMap<SocketAddr, Arc<Mutex<TcpStream>>>,
}

impl P2PServer {

    fn actor(message_channel: ChannelRef<P2PServerMsg>) -> Self {
        P2PServer {
            message_channel,
            inbound_connections: HashMap::new()
        }
    }
}

impl Actor for P2PServer {
    type Msg = P2PServerMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        // Use the respective Receive<T> implementation
        self.receive(ctx, msg, sender);
    }
}

impl Receive<HandleIpConnection> for P2PServer {
    type Msg = P2PServerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: HandleIpConnection, sender: Sender) {
        self.inbound_connections.insert(msg.address, msg.stream);
    }
}

impl Receive<BootstrapIp> for P2PServer {
    type Msg = P2PServerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: BootstrapIp, sender: Sender) {
        unimplemented!()
    }
}

impl Receive<DisconnectIp> for P2PServer {
    type Msg = P2PServerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: DisconnectIp, sender: Sender) {
        if let Some(stream) = self.inbound_connections.remove(&msg.address) {
            stream.lock().unwrap().shutdown(Shutdown::Write).unwrap();
        }
    }
}



pub async fn accept_incoming_p2p_connections(sys: &ActorSystem) -> Result<(), Box<dyn std::error::Error>> {

    let message_channel: ChannelRef<P2PServerMsg> = channel("p2p-channel", sys).unwrap();
    let props = Props::new_args(P2PServer::actor, message_channel);
    let p2p_server = sys.actor_of(props, "p2p-server").unwrap();


    let listener_address = "127.0.0.1:4578".parse()?;
    let mut listener = TcpListener::bind(&listener_address).unwrap();

    loop {
        let (stream, address) = listener.accept().await?;

        p2p_server.tell(IpConnected { stream: Arc::new(Mutex::new(stream)), address }, None);

//        let msg = IpConnected { address }.into();
//        message_channel.tell(Publish { msg, topic: "net".into() }, None);

//            tokio::spawn(async move {
//                let mut buf = [0; 1024];
//
//                // In a loop, read data from the socket and write the data back.
//                loop {
//                    let n = match socket.read(&mut buf).await {
//                        // socket closed
//                        Ok(n) if n == 0 => return,
//                        Ok(n) => n,
//                        Err(e) => {
//                            println!("failed to read from socket; err = {:?}", e);
//                            return;
//                        }
//                    };
//
//                    // Write the data back
//                    if let Err(e) = socket.write_all(&buf[0..n]).await {
//                        println!("failed to write to socket; err = {:?}", e);
//                        return;
//                    }
//                }
//            });
    }
}