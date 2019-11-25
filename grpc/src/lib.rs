use std::sync::Arc;
use slog::Logger;
use tonic::transport::Server;

mod server;
use server::TezedgeService;
use server::tezedge;
use tezedge::server::TezedgeServer;

// use failure::Fail;

// #[derive(Fail, Debug)]
// pub enum GrpcError {
//     #[fail(display = "Tonic error: {}", reason)]
//     TonicError {
//         reason: tonic::transport::error::Error
//     }
// }

// impl From<tonic::transport::error::Error> for GrpcError {
//     fn from(reason: tonic::transport::error::Error) -> Self {
//         GrpcError::TonicError { reason }
//     }
// }

pub async fn create_grpc_server(db: Arc<rocksdb::DB>, log: Logger) -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();

    let tezedge_service = TezedgeServer::new( TezedgeService{db : db, logger : log} );

    Server::builder()
        .add_service(tezedge_service)
        .serve(addr)
        .await?;

    Ok(())
}