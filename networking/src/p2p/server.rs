use std::collections::HashMap;
use std::net::SocketAddr;

use riker::actors::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

#[derive(Clone, Debug)]
pub struct IpConnected {
    address: SocketAddr
}

#[derive(Clone, Debug)]
pub struct BootstrapIp {
    address: SocketAddr
}

#[derive(Clone, Debug)]
pub struct DisconnectIp {
    address: SocketAddr
}



#[actor(IpConnected, BootstrapIp, DisconnectIp)]
pub struct P2PServer {
    sys: ActorSystem,
    inbound_connections: HashMap<SocketAddr, TcpStream>,
}

impl P2PServer {

    async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        let chan: ChannelRef<P2PServerMsg> = channel("p2p-server", &self.sys).unwrap();


        let listener_address = "127.0.0.1:8080".parse()?;
        let mut listener = TcpListener::bind(&listener_address).unwrap();

        loop {
            let (socket, address) = listener.accept().await?;

            self.inbound_connections.insert(address, socket);

//            let msg: P2PServerMsg = IpConnected { address }.into();
            chan.tell(Publish { msg: IpConnected { address }.into(), topic: "net".into() }, None);

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
}

impl Actor for P2PServer {
    type Msg = P2PServerMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        // Use the respective Receive<T> implementation
        self.receive(ctx, msg, sender);
    }
}

impl Receive<IpConnected> for P2PServer {
    type Msg = P2PServerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: IpConnected, sender: Sender) {
        unimplemented!()
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
        unimplemented!()
    }
}