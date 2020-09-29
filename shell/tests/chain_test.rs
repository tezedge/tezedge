// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]
extern crate test;

use std::sync::Once;
use std::time::Duration;

use lazy_static::lazy_static;
use slog::{info, Logger};

use shell::peer_manager::P2p;
use shell::PeerConnectionThreshold;
use tezos_identity::Identity;
use tezos_messages::p2p::encoding::version::NetworkVersion;

mod common;

lazy_static! {
    pub static ref NETWORK_VERSION: NetworkVersion = NetworkVersion::new("TEST_CHAIN".to_string(), 0, 0);
    pub static ref NODE_P2P_PORT: u16 = 1234; // TODO: maybe some logic to verify and get free port
    pub static ref NODE_P2P_CFG: (Identity, P2p, NetworkVersion) = (
        tezos_identity::Identity::generate(0f64),
        P2p {
            listener_port: NODE_P2P_PORT.clone(),
            bootstrap_lookup_addresses: vec![],
            disable_bootstrap_lookup: true,
            disable_mempool: false,
            private_node: false,
            initial_peers: vec![],
            peer_threshold: PeerConnectionThreshold::new(0, 10),
        },
        NETWORK_VERSION.clone(),
    );
}

fn init_data(log: &Logger) {
    static INIT_DATA: Once = Once::new();
    INIT_DATA.call_once(|| {
        info!(log, "Initializing test data...");
        let _ = NODE_P2P_CFG.clone();
        let _ = test_data::DB.block_hash(1);
        info!(log, "Test data initialized!");
    });
}

#[ignore]
#[test]
fn test_process_current_branch_on_level3_with_empty_storage() -> Result<(), failure::Error> {
    // logger
    let log_level = common::log_level();
    let log = common::create_logger(log_level.clone());

    init_data(&log);

    // start node
    let node = common::infra::NodeInfrastructure::start(
        "__test_01", "test_process_current_branch_on_level3_with_empty_storage",
        Some(NODE_P2P_CFG.clone()),
        (log, log_level),
    )?;

    // wait for storage initialization to genesis
    node.wait_for_new_current_head(node.tezos_env.genesis_header_hash()?, (Duration::from_secs(5), Duration::from_millis(250)))?;

    // connect mocked node peer with test data set
    let mocked_peer_node = test_node_peer::TestNodePeer::connect(
        NODE_P2P_CFG.1.listener_port,
        NODE_P2P_CFG.2.clone(),
        tezos_identity::Identity::generate(0f64),
        node.log.clone(),
        &node.tokio_runtime,
        test_cases_data::current_branch_on_level_3,
    );

    // wait for current head on level 3
    node.wait_for_new_current_head(test_data::DB.block_hash(3)?, (Duration::from_secs(30), Duration::from_millis(750)))?;

    // stop nodes
    drop(node);
    drop(mocked_peer_node);

    Ok(())
}

/// Stored first cca first 1300 apply block data
mod test_data {
    use std::{env, io};
    use std::collections::HashMap;
    use std::convert::TryInto;
    use std::fs::File;
    use std::io::BufRead;
    use std::path::Path;

    use failure::format_err;
    use itertools::Itertools;
    use lazy_static::lazy_static;

    use crypto::hash::{BlockHash, HashType};
    use tezos_api::environment::TezosEnvironment;
    use tezos_api::ffi::ApplyBlockRequest;
    use tezos_encoding::binary_reader::BinaryReader;
    use tezos_encoding::de::from_value as deserialize_from_value;
    use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
    use tezos_messages::p2p::binary_message::{BinaryMessage, MessageHash};
    use tezos_messages::p2p::encoding::block_header::Level;
    use tezos_messages::p2p::encoding::prelude::{BlockHeader, Operation, OperationsForBlock, OperationsForBlocksMessage};

    pub const TEZOS_NETWORK: TezosEnvironment = TezosEnvironment::Carthagenet;

    lazy_static! {
        pub static ref APPLY_BLOCK_REQUEST_ENCODING: Encoding = Encoding::Obj(vec![
            Field::new("chain_id", Encoding::Hash(HashType::ChainId)),
            Field::new("block_header", Encoding::dynamic(BlockHeader::encoding().clone())),
            Field::new("pred_header", Encoding::dynamic(BlockHeader::encoding().clone())),
            Field::new("max_operations_ttl", Encoding::Int31),
            Field::new("operations", Encoding::dynamic(Encoding::list(Encoding::dynamic(Encoding::list(Encoding::dynamic(Operation::encoding().clone())))))),
        ]);

        // prepared data - we have stored 1326 request for apply block
        pub static ref DB: Db = Db::init_db();
    }

    pub struct Db {
        requests: Vec<String>,
        headers: HashMap<BlockHash, Level>,
        operations: HashMap<OperationsForBlocksMessageKey, OperationsForBlocksMessage>,
    }

    impl Db {
        fn init_db() -> Db {
            let requests = read_apply_block_requests_until_1326();
            let mut headers: HashMap<BlockHash, Level> = HashMap::new();

            // init headers
            for (idx, request) in requests.iter().enumerate() {
                let request = from_captured_bytes(request).expect("Failed to parse request");
                let block = request.block_header.message_hash().expect("Failed to decode message_hash");
                headers.insert(block, to_level(idx));
            }

            Db {
                requests,
                headers,
                operations: read_operations_for_blocks_message_until_1328(),
            }
        }

        pub fn get(&self, block_hash: &BlockHash) -> Result<Option<BlockHeader>, failure::Error> {
            match DB.headers.get(block_hash) {
                Some(level) => {
                    Ok(Some(self.from_captured_requests(level.clone())?.block_header))
                }
                None => Ok(None)
            }
        }

        pub fn get_operations_for_block(&self, block: &OperationsForBlock) -> Result<Option<OperationsForBlocksMessage>, failure::Error> {
            match DB.operations.get(&OperationsForBlocksMessageKey::new(block.block_hash().clone(), block.validation_pass())) {
                Some(operations) => {
                    Ok(Some(operations.clone()))
                }
                None => Ok(None)
            }
        }

        pub fn block_hash(&self, level: Level) -> Result<BlockHash, failure::Error> {
            let block_hash = self.headers
                .iter()
                .find(|(_, value)| level.eq(*value))
                .map(|(k, _)| k.clone());
            match block_hash {
                Some(block_hash) => Ok(block_hash),
                None => Err(format_err!("No header found for level: {}", level))
            }
        }

        /// Create new struct from captured requests by level.
        pub fn from_captured_requests(&self, level: Level) -> Result<ApplyBlockRequest, failure::Error> {
            from_captured_bytes(&self.requests[to_index(level)])
        }
    }

    /// requests are indexed from 0, so [0] is level 1, [1] is level 2, and so on ...
    fn to_index(level: Level) -> usize {
        (level - 1).try_into().expect("Failed to convert level to usize")
    }

    fn to_level(idx: usize) -> Level {
        (idx + 1).try_into().expect("Failed to convert index to Level")
    }

    /// Create new struct from bytes.
    #[inline]
    fn from_captured_bytes(request: &str) -> Result<ApplyBlockRequest, failure::Error> {
        let bytes = hex::decode(request)?;
        let value = BinaryReader::new().read(bytes, &APPLY_BLOCK_REQUEST_ENCODING)?;
        let value: ApplyBlockRequest = deserialize_from_value(&value)?;
        Ok(value)
    }

    pub fn read_apply_block_requests_until_1326() -> Vec<String> {
        let path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("tests")
            .join("resources")
            .join("apply_block_request_until_1326.zip");
        let file = File::open(path).expect("Couldn't open file: tests/resources/apply_block_request_until_1326.zip");
        let mut archive = zip::ZipArchive::new(file).unwrap();

        let mut requests: Vec<String> = Vec::new();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let mut writer: Vec<u8> = vec![];
            io::copy(&mut file, &mut writer).unwrap();
            requests.push(String::from_utf8(writer).expect("error"));
        }

        requests
    }

    pub fn read_operations_for_blocks_message_until_1328() -> HashMap<OperationsForBlocksMessageKey, OperationsForBlocksMessage> {
        let path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("tests")
            .join("resources")
            .join("OperationsForBlocksMessage.zip");
        let file = File::open(path).expect("Couldn't open file: tests/resources/OperationsForBlocksMessage.zip");
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let file = archive.by_name("OperationsForBlocksMessage").unwrap();

        let mut operations: HashMap<OperationsForBlocksMessageKey, OperationsForBlocksMessage> = HashMap::new();

        // read file by lines
        let reader = io::BufReader::new(file);
        let lines = reader.lines();
        for line in lines {
            if let Ok(mut line) = line {
                let _ = line.remove(0);
                let split = line.split("|").collect_vec();
                assert_eq!(3, split.len());

                let block_hash = HashType::BlockHash.string_to_bytes(split[0]).expect("Failed to parse block_hash");
                let validation_pass = split[1].parse::<i8>().expect("Failed to parse validation_pass");

                let operations_for_blocks_message = hex::decode(split[2]).expect("Failed to parse operations_for_blocks_message");
                let operations_for_blocks_message = OperationsForBlocksMessage::from_bytes(operations_for_blocks_message).expect("Failed to readed bytes for operations_for_blocks_message");

                operations.insert(
                    OperationsForBlocksMessageKey::new(block_hash, validation_pass),
                    operations_for_blocks_message,
                );
            }
        }

        operations
    }

    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct OperationsForBlocksMessageKey {
        block_hash: String,
        validation_pass: i8,
    }

    impl OperationsForBlocksMessageKey {
        pub fn new(block_hash: BlockHash, validation_pass: i8) -> Self {
            OperationsForBlocksMessageKey {
                block_hash: HashType::BlockHash.bytes_to_string(&block_hash),
                validation_pass,
            }
        }
    }
}

/// Predefined data sets as callback functions for test node peer
mod test_cases_data {
    use crypto::hash::BlockHash;
    use tezos_messages::p2p::encoding::prelude::{BlockHeaderMessage, CurrentBranch, CurrentBranchMessage, PeerMessage, PeerMessageResponse};

    use crate::test_data::DB;

    pub fn current_branch_on_level_3(message: PeerMessageResponse) -> Result<Vec<PeerMessageResponse>, failure::Error> {
        full_data(message, Some(DB.block_hash(3)?))
    }

    pub fn full_data(message: PeerMessageResponse, desired_current_branch: Option<BlockHash>) -> Result<Vec<PeerMessageResponse>, failure::Error> {
        match message.messages().get(0).unwrap() {
            PeerMessage::GetCurrentBranch(request) => {
                match desired_current_branch {
                    Some(block_hash) => {
                        if let Some(block_header) = DB.get(&block_hash)? {
                            let current_branch = CurrentBranchMessage::new(
                                request.chain_id.clone(),
                                CurrentBranch::new(
                                    block_header.clone(),
                                    vec![
                                        block_hash,
                                        block_header.predecessor().clone(),
                                    ],
                                ),
                            );
                            Ok(vec![current_branch.into()])
                        } else {
                            Ok(vec![])
                        }
                    }
                    None => Ok(vec![])
                }
            }
            PeerMessage::GetBlockHeaders(request) => {
                let mut responses: Vec<PeerMessageResponse> = Vec::new();
                for block_hash in request.get_block_headers() {
                    if let Some(block_header) = DB.get(block_hash)? {
                        let msg: BlockHeaderMessage = block_header.into();
                        responses.push(msg.into());
                    }
                }
                Ok(responses)
            }
            PeerMessage::GetOperationsForBlocks(request) => {
                let mut responses: Vec<PeerMessageResponse> = Vec::new();
                for block in request.get_operations_for_blocks() {
                    if let Some(msg) = DB.get_operations_for_block(block)? {
                        responses.push(msg.into());
                    }
                }
                Ok(responses)
            }
            _ => Ok(vec![])
        }
    }
}

/// Test node peer, which simulates p2p remote peer, communicates through real p2p socket
mod test_node_peer {
    use std::net::{Shutdown, SocketAddr};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use slog::{crit, debug, error, info, Logger, warn};
    use tokio::net::TcpStream;
    use tokio::runtime::Runtime;
    use tokio::time::timeout;

    use networking::p2p::peer;
    use networking::p2p::peer::{Bootstrap, BootstrapOutput, Local};
    use tezos_identity::Identity;
    use tezos_messages::p2p::encoding::prelude::{PeerMessage, PeerMessageResponse};
    use tezos_messages::p2p::encoding::version::NetworkVersion;

    const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
    const READ_TIMEOUT_LONG: Duration = Duration::from_secs(30);

    pub struct TestNodePeer {
        run: Arc<AtomicBool>,
    }

    impl TestNodePeer {
        pub fn connect(
            connect_to_node_port: u16,
            network_version: NetworkVersion,
            identity: Identity,
            log: Logger,
            tokio_runtime: &Runtime,
            handle_message_callback: fn(PeerMessageResponse) -> Result<Vec<PeerMessageResponse>, failure::Error>) -> TestNodePeer {
            let server_address = format!("0.0.0.0:{}", connect_to_node_port).parse::<SocketAddr>().expect("Failed to parse server address");
            let tokio_executor = tokio_runtime.handle().clone();
            let run = Arc::new(AtomicBool::new(false));

            {
                let run = run.clone();
                tokio_executor.spawn(async move {
                    // init socket connection to server node
                    match timeout(CONNECT_TIMEOUT, TcpStream::connect(&server_address)).await {
                        Ok(Ok(stream)) => {
                            info!(log, "[TEST_PEER_NODE] Connection successful"; "ip" => server_address);

                            // authenticate
                            let local = Arc::new(Local::new(
                                1235,
                                identity.public_key,
                                identity.secret_key,
                                identity.proof_of_work_stamp,
                                network_version,
                            ));
                            let bootstrap = Bootstrap::outgoing(
                                stream,
                                server_address,
                                false,
                                false,
                            );

                            let bootstrap_result = peer::bootstrap(
                                bootstrap,
                                local,
                                &log,
                            ).await.expect("[TEST_PEER_NODE] Failed to bootstrap");

                            // process messages
                            run.store(true, Ordering::Release);
                            Self::begin_process_incoming(bootstrap_result, run, log, server_address, handle_message_callback).await;
                        }
                        Ok(Err(e)) => {
                            error!(log, "[TEST_PEER_NODE] Connection failed"; "ip" => server_address, "reason" => format!("{:?}", e));
                        }
                        Err(_) => {
                            error!(log, "[TEST_PEER_NODE] Connection timed out"; "ip" => server_address);
                        }
                    }
                });
            }

            TestNodePeer {
                run
            }
        }

        /// Start to process incoming data
        async fn begin_process_incoming(
            bootstrap: BootstrapOutput,
            run: Arc<AtomicBool>,
            log: Logger,
            peer_address: SocketAddr,
            handle_message_callback: fn(PeerMessageResponse) -> Result<Vec<PeerMessageResponse>, failure::Error>) {
            info!(log, "[TEST_PEER_NODE] Starting to accept messages"; "ip" => format!("{:?}", &peer_address));
            let BootstrapOutput(mut rx, mut tx, ..) = bootstrap;

            while run.load(Ordering::Acquire) {
                match timeout(READ_TIMEOUT_LONG, rx.read_message::<PeerMessageResponse>()).await {
                    Ok(res) => match res {
                        Ok(msg) => {
                            let msg_type = msg_type(&msg);
                            info!(log, "[TEST_PEER_NODE] Handle message"; "ip" => format!("{:?}", &peer_address), "msg_type" => msg_type.clone());

                            // apply callback
                            match handle_message_callback(msg) {
                                Ok(responses) => {
                                    info!(log, "[TEST_PEER_NODE] Message handled({})", !responses.is_empty(); "msg_type" => msg_type);
                                    for response in responses {
                                        // send back response
                                        tx.write_message(&response).await.expect("[TEST_PEER_NODE] Failed to send message");
                                    };
                                }
                                Err(e) => error!(log, "[TEST_PEER_NODE] Failed to handle message"; "reason" => format!("{:?}", e), "msg_type" => msg_type)
                            }
                        }
                        Err(e) => {
                            crit!(log, "[TEST_PEER_NODE] Failed to read peer message"; "reason" => e);
                            break;
                        }
                    }
                    Err(_) => {
                        warn!(log, "[TEST_PEER_NODE] Peer message read timed out"; "secs" => READ_TIMEOUT_LONG.as_secs());
                        break;
                    }
                }
            }

            debug!(log, "[TEST_PEER_NODE] Shutting down peer connection"; "ip" => format!("{:?}", &peer_address));
            // let mut tx_lock = tx.lock().await;
            // if let Some(tx) = tx_lock.take() {
            let socket = rx.unsplit(tx);
            match socket.shutdown(Shutdown::Both) {
                Ok(()) => debug!(log, "[TEST_PEER_NODE] Connection shutdown successful"; "socket" => format!("{:?}", socket)),
                Err(err) => debug!(log, "[TEST_PEER_NODE] Failed to shutdown connection"; "err" => format!("{:?}", err), "socket" => format!("{:?}", socket)),
            }
            // }

            info!(log, "[TEST_PEER_NODE] Stopped to accept messages"; "ip" => format!("{:?}", &peer_address));
        }

        pub fn stop(&mut self) {
            self.run.store(false, Ordering::Release);
        }
    }

    impl Drop for TestNodePeer {
        fn drop(&mut self) {
            self.stop();
        }
    }

    fn msg_type(msg: &PeerMessageResponse) -> String {
        msg.messages()
            .iter()
            .map(|m| match m {
                PeerMessage::Disconnect => "Disconnect",
                PeerMessage::Advertise(_) => "Advertise",
                PeerMessage::SwapRequest(_) => "SwapRequest",
                PeerMessage::SwapAck(_) => "SwapAck",
                PeerMessage::Bootstrap => "Bootstrap",
                PeerMessage::GetCurrentBranch(_) => "GetCurrentBranch",
                PeerMessage::CurrentBranch(_) => "CurrentBranch",
                PeerMessage::Deactivate(_) => "Deactivate",
                PeerMessage::GetCurrentHead(_) => "GetCurrentHead",
                PeerMessage::CurrentHead(_) => "CurrentHead",
                PeerMessage::GetBlockHeaders(_) => "GetBlockHeaders",
                PeerMessage::BlockHeader(_) => "BlockHeader",
                PeerMessage::GetOperations(_) => "GetOperations",
                PeerMessage::Operation(_) => "Operation",
                PeerMessage::GetProtocols(_) => "GetProtocols",
                PeerMessage::Protocol(_) => "Protocol",
                PeerMessage::GetOperationHashesForBlocks(_) => "GetOperationHashesForBlocks",
                PeerMessage::OperationHashesForBlock(_) => "OperationHashesForBlock",
                PeerMessage::GetOperationsForBlocks(_) => "GetOperationsForBlocks",
                PeerMessage::OperationsForBlocks(_) => "OperationsForBlocks",
            })
            .collect::<Vec<&str>>()
            .join(",")
    }
}
