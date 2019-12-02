
use slog::{Logger, debug, info};
use tonic::{Request, Response, Status};
use std::sync::Arc;

pub mod tezedge {
    // The string specified here must match the proto package name
    tonic::include_proto!("tezedge"); 
}

use tezedge::{
    server::{Tezedge},
    HelloReply, HelloRequest, GetBlockRequest, GetBlockReply, MonitorCommitHashRequest, MonitorCommitHashReply
};

pub struct TezedgeService {
    pub db: Arc<rocksdb::DB>,
    pub logger: Logger
}

#[tonic::async_trait]
impl Tezedge for TezedgeService {
    async fn say_hello(&self, request: Request<HelloRequest>,) -> Result<Response<HelloReply>, Status> {
        debug!(self.logger, "Got a Hello request: {:?}", request);

        let reply = HelloReply {
            message: format!("Hello {}!", request.into_inner().name).into(), // We must use .into_inner() as the fields of gRPC requests and responses are private
        };

        Ok(Response::new(reply))
    }

    async fn get_block(&self, request: Request<GetBlockRequest>,) -> Result<Response<GetBlockReply>, Status> {
        info!(self.logger, "Got a GetBlock request: {:?}", request);

        let reply = GetBlockReply {
            block_hash: format!("block_hash: unknown").into(), // We must use .into_inner() as the fields of gRPC requests and responses are private
            chain_id: format!("chain_id: unknown").into(), // We must use .into_inner() as the fields of gRPC requests and responses are private
        };

        // Example how to transform protobuf to json
        let json_reply = serde_json::to_string(&reply).unwrap();
        info!(self.logger, "GetBlock json response: {:?}", json_reply);


        Ok(Response::new(reply))
    }

    async fn monitor_commit_hash(&self, request: Request<MonitorCommitHashRequest>,) -> Result<Response<MonitorCommitHashReply>, Status> {
        info!(self.logger, "Got a MonitorCommitHash request: {:?}", request);

        let reply = MonitorCommitHashReply {
            commit_hash: env!("GIT_HASH").into(), // We must use .into_inner() as the fields of gRPC requests and responses are private
        };

        Ok(Response::new(reply))
    }
}