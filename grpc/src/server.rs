use tonic::{transport::Server, Request, Response, Status};

pub mod tezedge {
    tonic::include_proto!("tezedge"); // The string specified here must match the proto package name
}

use tezedge::{
    server::{Tezedge, TezedgeServer},
    HelloReply, HelloRequest,
};

pub struct MyTezedge {}

#[tonic::async_trait]
impl Tezedge for MyTezedge {
    async fn say_hello(
        &self,
        request: Request<HelloRequest>, // Accept request of type HelloRequest
    ) -> Result<Response<HelloReply>, Status> { // Return an instance of type HelloReply
        println!("Got a request: {:?}", request);

        let reply = tezedge::HelloReply {
            message: format!("Hello {}!", request.into_inner().name).into(), // We must use .into_inner() as the fields of gRPC requests and responses are private
        };

        Ok(Response::new(reply)) // Send back our formatted greeting
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let tezedge = MyTezedge {};    

    Server::builder()
        .add_service(TezedgeServer::new(tezedge))
        .serve(addr)
        .await?;

    Ok(())
}