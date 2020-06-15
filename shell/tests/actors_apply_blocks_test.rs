// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]
extern crate test;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use jsonpath::Selector;
use riker::actors::*;
use riker::system::SystemBuilder;
use serde_json::Value;
use slog::Logger;

use crypto::hash::HashType;
use shell::chain_feeder::ChainFeeder;
use shell::context_listener::ContextListener;
use shell::shell_channel::{ShellChannel, ShellChannelTopic, ShuttingDown};
use storage::{BlockHeaderWithHash, BlockMetaStorage, BlockMetaStorageReader, BlockStorage, BlockStorageReader, OperationsMetaStorage, OperationsStorage, resolve_storage_init_chain_data};
use storage::context::{ContextApi, ContextIndex, TezedgeContext};
use storage::persistent::{ContextList, PersistentStorage};
use storage::skip_list::Bucket;
use storage::tests_common::TmpStorage;
use tezos_api::environment::{TEZOS_ENV, TezosEnvironmentConfiguration};
use tezos_api::ffi::{ApplyBlockRequest, FfiMessage, RustBytes, TezosRuntimeConfiguration};
use tezos_messages::p2p::binary_message::MessageHash;
use tezos_messages::p2p::encoding::operations_for_blocks::{OperationsForBlock, OperationsForBlocksMessage};
use tezos_messages::p2p::encoding::operations_for_blocks;
use tezos_wrapper::service::{ProtocolEndpointConfiguration, ProtocolRunner, ProtocolRunnerEndpoint};

mod common;

#[test]
fn test_actors_apply_blocks_and_check_context() -> Result<(), failure::Error> {

    // logger
    let log_level = common::log_level();
    let log = common::create_logger(log_level.clone());

    // environement
    let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV.get(&test_data::TEZOS_NETWORK).expect("no environment configuration");

    // storage
    let storage_db_path = "__shell_context_listener_test_apply_blocks";
    let context_db_path = common::prepare_empty_dir("__shell_context_listener_test_apply_blocks_context");
    let tmp_storage = TmpStorage::create(common::prepare_empty_dir(storage_db_path))?;
    let persistent_storage = tmp_storage.storage();
    let mut block_storage = BlockStorage::new(&persistent_storage);
    let mut block_meta_storage = BlockMetaStorage::new(&persistent_storage);
    let mut operations_storage = OperationsStorage::new(&persistent_storage);
    let mut operations_meta_storage = OperationsMetaStorage::new(&persistent_storage);

    let storage_db_path = PathBuf::from(storage_db_path);
    let context_db_path = PathBuf::from(context_db_path);
    let init_storage_data = resolve_storage_init_chain_data(&tezos_env, &storage_db_path, &context_db_path, log.clone())
        .expect("Failed to resolve init storage chain data");

    // protocol runner endpoint
    let protocol_runner = PathBuf::from("no_executable_protocol_runnner");
    let protocol_runner_endpoint = ProtocolRunnerEndpoint::<common::TestProtocolRunner>::new(
        "test_protocol_runner_endpoint",
        ProtocolEndpointConfiguration::new(
            TezosRuntimeConfiguration {
                log_enabled: false,
                no_of_ffi_calls_treshold_for_gc: 50,
                debug_mode: false,
            },
            tezos_env.clone(),
            false,
            &context_db_path,
            &protocol_runner,
            log_level,
        ),
        log.clone(),
    );
    let (protocol_commands, protocol_events) = match protocol_runner_endpoint.start() {
        Ok(_) => {
            let ProtocolRunnerEndpoint {
                commands,
                events,
                ..
            } = protocol_runner_endpoint;
            (commands, events)
        }
        Err(e) => panic!("Error to start test protocol runner: {:?}", e)
    };

    // run actor's
    let actor_system = SystemBuilder::new().name("test_apply_block_and_check_context").log(log.clone()).create().expect("Failed to create actor system");
    let shell_channel = ShellChannel::actor(&actor_system).expect("Failed to create shell channel");
    let _ = ContextListener::actor(&actor_system, &persistent_storage, protocol_events, log.clone(), false).expect("Failed to create context event listener");
    let _ = ChainFeeder::actor(&actor_system, shell_channel.clone(), &persistent_storage, &init_storage_data, &tezos_env, protocol_commands, log.clone()).expect("Failed to create chain feeder");

    // prepare data for apply blocks and wait for current head
    assert!(
        apply_blocks_with_chain_feeder(
            &mut block_storage,
            &mut block_meta_storage,
            &mut operations_storage,
            &mut operations_meta_storage,
            tezos_env,
            log.clone(),
        ).is_ok()
    );

    // clean up
    // shutdown events listening
    thread::sleep(Duration::from_secs(3));
    shell_channel.tell(
        Publish {
            msg: ShuttingDown.into(),
            topic: ShellChannelTopic::ShellCommands.into(),
        }, None,
    );
    thread::sleep(Duration::from_secs(2));
    let _ = actor_system.shutdown();

    // check context
    check_context(&persistent_storage)
}

fn check_context(persistent_storage: &PersistentStorage) -> Result<(), failure::Error> {
    let context = TezedgeContext::new(
        BlockStorage::new(&persistent_storage),
        persistent_storage.context_storage(),
    );

    // check level 0
    if let Some(Bucket::Exists(data)) = context.get_key(&ContextIndex::new(Some(0), None), &vec!["protocol".to_string()])? {
        assert_eq!("PtYuensgYBb3G3x1hLLbCmcav8ue8Kyd2khADcL5LsT5R1hcXex", HashType::ProtocolHash.bytes_to_string(&data));
    } else {
        panic!(format!("Protocol not found in context for level: {}", 0));
    }

    // check level 1
    if let Some(Bucket::Exists(data)) = context.get_key(&ContextIndex::new(Some(1), None), &vec!["protocol".to_string()])? {
        assert_eq!("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS", HashType::ProtocolHash.bytes_to_string(&data));
    } else {
        panic!(format!("Protocol not found in context for level: {}", 1));
    }

    // check level 2
    if let Some(Bucket::Exists(data)) = context.get_key(&ContextIndex::new(Some(2), None), &vec!["protocol".to_string()])? {
        assert_eq!("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS", HashType::ProtocolHash.bytes_to_string(&data));
    } else {
        panic!(format!("Protocol not found in context for level: {}", 2));
    }

    let context_list: ContextList = persistent_storage.context_storage();
    let list = context_list.read().expect("lock poisoning");

    // check level 131 - total compare
    let ctxt = list.get(131)?;
    assert!(ctxt.is_some());
    assert_ctxt(ctxt.unwrap(), test_data::read_context_json("context_131.json").expect("context_131.json not found"));

    // check level 181 - total compare
    let ctxt = list.get(181)?;
    assert!(ctxt.is_some());
    assert_ctxt(ctxt.unwrap(), test_data::read_context_json("context_181.json").expect("context_181.json not found"));

    // check level 553 - total compare
    let ctxt = list.get(553)?;
    assert!(ctxt.is_some());
    assert_ctxt(ctxt.unwrap(), test_data::read_context_json("context_553.json").expect("context_553.json not found"));

    // check level 834 - total compare
    let ctxt = list.get(834)?;
    assert!(ctxt.is_some());
    assert_ctxt(ctxt.unwrap(), test_data::read_context_json("context_834.json").expect("context_834.json not found"));

    // check level 1322 - total compare
    let ctxt = list.get(1322)?;
    assert!(ctxt.is_some());
    assert_ctxt(ctxt.unwrap(), test_data::read_context_json("context_1322.json").expect("context_1322.json not found"));

    Ok(())
}

fn assert_ctxt(ctxt: HashMap<String, Bucket<Vec<u8>>>, ocaml_ctxt_as_json: String) {
    let json: Value = serde_json::from_str(&ocaml_ctxt_as_json).unwrap();
    for (key, value) in ctxt.iter() {
        // comparing just data
        if !key.starts_with("data") {
            continue;
        }

        let key_selector = key.replacen("data/", "", 1).replace("/", ".");
        let selector = Selector::new(&format!("$.{}", key_selector)).unwrap();
        let ocaml_data: Vec<&str> = selector
            .find(&json)
            .map(|t| t.as_str().unwrap())
            .collect();

        match value {
            Bucket::Deleted => {
                assert!(ocaml_data.is_empty())
            }
            Bucket::Exists(rust_data) => {
                let rust_data = &hex::encode(rust_data);
                assert_eq!(rust_data, ocaml_data[0])
            }
        };
    }
}

fn apply_blocks_with_chain_feeder(
    block_storage: &mut BlockStorage,
    block_meta_storage: &mut BlockMetaStorage,
    operations_storage: &mut OperationsStorage,
    operations_meta_storage: &mut OperationsMetaStorage,
    tezos_env: &TezosEnvironmentConfiguration,
    log: Logger) -> Result<(), failure::Error> {
    let chain_id = tezos_env.main_chain_id().expect("invalid chain id");

    // let's insert stored requests to database
    for request in test_data::apply_block_requests_until_1326() {

        // parse request
        let request: RustBytes = hex::decode(request)?;
        let request = ApplyBlockRequest::from_rust_bytes(request)?;
        let header = request.block_header.clone();

        // store header to db
        let block = BlockHeaderWithHash {
            hash: header.message_hash()?,
            header: Arc::new(header),
        };
        block_storage.put_block_header(&block)?;
        block_meta_storage.put_block_header(&block, &chain_id, log.clone())?;
        operations_meta_storage.put_block_header(&block, &chain_id)?;

        // store operations to db
        let operations = request.operations.clone();
        for (idx, ops) in operations.iter().enumerate() {
            let opb = OperationsForBlock::new(block.hash.clone(), idx as i8);
            let msg: OperationsForBlocksMessage = OperationsForBlocksMessage::new(opb, operations_for_blocks::Path::Op, ops.clone());
            operations_storage.put_operations(&msg)?;
            operations_meta_storage.put_operations(&msg)?;
        }
        assert!(operations_meta_storage.is_complete(&block.hash)?);
    }

    // wait for applied blocks
    loop {
        let head = block_meta_storage.load_current_head()?;
        match head {
            None => (),
            Some((h, _)) => {
                let meta = block_meta_storage.get(&h)?;
                if let Some(m) = meta {
                    let header = block_storage.get(&h).expect("failed to read current head").expect("current head not found");
                    if m.level() >= 1326 {
                        // TE-168: check if context is also asynchronously stored
                        let context_hash = header.header.context();
                        let found_by_context_hash = block_storage.get_by_context_hash(&context_hash).expect("failed to read head");
                        if found_by_context_hash.is_some() {
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

mod test_data {
    use std::{env, io};
    use std::fs::File;
    use std::path::Path;

    use tezos_api::environment::TezosEnvironment;

    pub const TEZOS_NETWORK: TezosEnvironment = TezosEnvironment::Carthagenet;

    pub fn apply_block_requests_until_1326() -> Vec<String> {
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

    pub fn read_context_json(file_name: &str) -> Option<String> {
        let path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("tests")
            .join("resources")
            .join("ocaml_context_jsons.zip");
        let file = File::open(path).expect("Couldn't open file: tests/resources/ocaml_context_jsons.zip");
        let mut archive = zip::ZipArchive::new(file).unwrap();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            if file.name().eq(file_name) {
                let mut writer: Vec<u8> = vec![];
                io::copy(&mut file, &mut writer).unwrap();
                return Some(String::from_utf8(writer).expect("error"));
            }
        }

        None
    }
}
