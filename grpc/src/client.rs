pub mod tezedge {
    // The string specified here must match the proto package name
    tonic::include_proto!("tezedge"); 
}

use tezedge::{
    client::{TezedgeClient},
    HelloRequest, ChainsBlocksRequest, MonitorCommitHashRequest
};

use tonic::transport::Endpoint;


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let channel = Endpoint::from_static("http://[::1]:50051")
        .connect()
        .await?;

    let mut tezedge_client = TezedgeClient::new(channel);

    let request = tonic::Request::new(HelloRequest {
        name: "Tonic".into(),
    });
    println!("Sending say_hello Request={:?}", request);
    let response = tezedge_client.say_hello(request).await?;
    println!("say_hello Response={:?}", response);


    let request = tonic::Request::new(ChainsBlocksRequest {
        chain_id: "some_chain_id".into(),
        block_id: "some_block_id".into(),
    });
    println!("Sending chains_blocks Request={:?}", request);
    let response = tezedge_client.chains_blocks(request).await?;
    println!("chains_blocks Response={:?}", response);


    let request = tonic::Request::new(MonitorCommitHashRequest {});
    println!("Sending monitor_commit_hash Request={:?}", request);
    let response = tezedge_client.monitor_commit_hash(request).await?;
    println!("monitor_commit_hash Response={:?}", response);


    Ok(())
}