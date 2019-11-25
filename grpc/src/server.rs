use slog::{Logger, debug};
use tonic::{Request, Response, Status};
use std::sync::Arc;

pub mod tezedge {
    // The string specified here must match the proto package name
    tonic::include_proto!("tezedge"); 
}

use tezedge::{
    server::{Tezedge},
    HelloReply, HelloRequest, ChainsBlocksRequest, ChainsBlocksReply, MonitorCommitHashRequest, MonitorCommitHashReply
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

    async fn chains_blocks(&self, request: Request<ChainsBlocksRequest>,) -> Result<Response<ChainsBlocksReply>, Status> {
        debug!(self.logger, "Got a ChainsBlocks request: {:?}", request);

        let reply = ChainsBlocksReply {
            block_hash: format!("block_hash: unknown").into(), // We must use .into_inner() as the fields of gRPC requests and responses are private
        };

        Ok(Response::new(reply))
    }

    async fn monitor_commit_hash(&self, request: Request<MonitorCommitHashRequest>,) -> Result<Response<MonitorCommitHashReply>, Status> {
        debug!(self.logger, "Got a MonitorCommitHash request: {:?}", request);

        let reply = MonitorCommitHashReply {
            commit_hash: env!("GIT_HASH").into(), // We must use .into_inner() as the fields of gRPC requests and responses are private
        };

        Ok(Response::new(reply))
    }
}