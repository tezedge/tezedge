// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]
extern crate test;

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

use jsonpath::Selector;
use riker::system::SystemBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slog::{Drain, Level, Logger, warn};

use crypto::hash::{BlockHash, ChainId, ContextHash, HashType};
use shell::context_listener::ContextListener;
use storage::{BlockHeaderWithHash, BlockMetaStorage, BlockStorage, initialize_storage_with_genesis_block, OperationsMetaStorage, resolve_storage_init_chain_data, store_commit_genesis_result};
use storage::context::{ContextApi, ContextIndex, TezedgeContext};
use storage::persistent::ContextList;
use storage::skip_list::Bucket;
use storage::tests_common::TmpStorage;
use tezos_api::environment::{TEZOS_ENV, TezosEnvironmentConfiguration};
use tezos_api::ffi::{APPLY_BLOCK_REQUEST_ENCODING, ApplyBlockError, ApplyBlockRequest, ApplyBlockResult, RustBytes, TezosRuntimeConfiguration};
use tezos_client::client;
use tezos_context::channel::*;
use tezos_encoding::binary_reader::BinaryReader;
use tezos_encoding::de;
use tezos_interop::ffi;
use tezos_messages::p2p::binary_message::MessageHash;
use tezos_wrapper::service::IpcEvtServer;

#[test]
fn test_apply_block_and_check_context() -> Result<(), failure::Error> {

    // logger
    let log = create_logger();

    // storage
    let storage_db_path = "__shell_context_listener_test_apply_blocks";
    let context_db_path = "__shell_context_listener_test_apply_blocks_context";
    let tmp_storage = TmpStorage::create(common::prepare_empty_dir(storage_db_path))?;
    let persistent_storage = tmp_storage.storage();
    let mut block_storage = BlockStorage::new(&persistent_storage);
    let mut block_meta_storage = BlockMetaStorage::new(&persistent_storage);
    let mut operations_meta_storage = OperationsMetaStorage::new(&persistent_storage);

    // environement
    let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV.get(&test_data::TEZOS_NETWORK).expect("no environment configuration");

    // run event ipc server
    let event_server = IpcEvtServer::new();
    let evt_socket_path = event_server.client_path();

    // run context_event callback listener
    let event_thread = {
        let log = log.clone();
        let evt_socket_path = evt_socket_path.clone();
        // enable context event to receive
        enable_context_channel();
        thread::spawn(move || {
            match tezos_wrapper::service::process_protocol_events(&evt_socket_path) {
                Ok(()) => (),
                Err(err) => {
                    warn!(log, "Error while processing protocol events"; "reason" => format!("{:?}", err))
                }
            }
        })
    };

    // run context_listener actor
    let actor_system = SystemBuilder::new().name("test_apply_block_and_check_context").log(log.clone()).create().expect("Failed to create actor system");
    let _ = ContextListener::actor(&actor_system, &persistent_storage, event_server, log.clone()).expect("Failed to create context event listener");

    // run apply blocks
    let _ = apply_blocks_like_chain_feeder(
        &mut block_storage,
        &mut block_meta_storage,
        &mut operations_meta_storage,
        tezos_env,
        storage_db_path,
        context_db_path,
        log.clone(),
    )?;
    // assert!(result.is_ok());

    // clean up
    // shutdown events listening
    tezos_context::channel::context_send(ContextAction::Shutdown)?;

    assert!(event_thread.join().is_ok());
    let _ = actor_system.shutdown();

    // check context 0/1/2
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

fn apply_blocks_like_chain_feeder(
    block_storage: &mut BlockStorage,
    block_meta_storage: &mut BlockMetaStorage,
    operations_meta_storage: &mut OperationsMetaStorage,
    tezos_env: &TezosEnvironmentConfiguration,
    _storage_db_path: &str,
    context_db_path: &str,
    log: Logger) -> Result<(), failure::Error> {
    ffi::change_runtime_configuration(
        TezosRuntimeConfiguration {
            log_enabled: common::is_ocaml_log_enabled(),
            no_of_ffi_calls_treshold_for_gc: common::no_of_ffi_calls_treshold_for_gc(),
        }
    ).unwrap().unwrap();

    // init context
    let init_storage_data = resolve_storage_init_chain_data(
        &tezos_env,
        log.clone(),
    )?;
    let (chain_id, .., genesis_context_hash) = init_test_protocol_context(tezos_env, context_db_path);

    // store genesis
    let _ = initialize_storage_with_genesis_block(
        block_storage,
        &init_storage_data,
        &tezos_env,
        &genesis_context_hash,
        log.clone(),
    )?;

    // store genesis result
    let commit_data = client::genesis_result_data(
        &genesis_context_hash,
        &chain_id,
        &tezos_env.genesis_protocol()?,
        tezos_env.genesis_additional_data().max_operations_ttl,
    )?;
    let _ = store_commit_genesis_result(
        block_storage,
        block_meta_storage,
        operations_meta_storage,
        &init_storage_data,
        commit_data,
    )?;

    // let's apply stored requests
    let reader = BinaryReader::new();
    let mut last_result: Option<ApplyBlockResult> = None;
    for request in test_data::apply_block_requests_until_1326() {

        // parse request
        let request: RustBytes = hex::decode(request)?;
        let decoded_request = reader.read(&request, &APPLY_BLOCK_REQUEST_ENCODING)?;
        let decoded_request: ApplyBlockRequest = de::from_value(&decoded_request)?;
        let header = decoded_request.block_header;

        // store header to db
        let block = BlockHeaderWithHash {
            hash: header.message_hash()?,
            header: Arc::new(header),
        };
        block_storage.put_block_header(&block)?;

        // apply request
        let result = match ffi::apply_block(request) {
            Ok(result) => result,
            Err(e) => {
                Err(ApplyBlockError::FailedToApplyBlock {
                    message: format!("Unknown OcamlError: {:?}", e)
                })
            }
        };
        assert!(result.is_ok());
        last_result = Some(result.unwrap());
    }

    Ok(assert!(last_result.is_some()))
}

fn init_test_protocol_context(tezos_env: &TezosEnvironmentConfiguration, dir_name: &str) -> (ChainId, BlockHash, ContextHash) {
    let result = client::init_protocol_context(
        common::prepare_empty_dir(dir_name),
        tezos_env.genesis.clone(),
        tezos_env.protocol_overrides.clone(),
        true,
        false,
    ).unwrap();

    let genesis_context_hash = match result.genesis_commit_hash {
        None => panic!("we needed commit_genesis and here should be result of it"),
        Some(cr) => cr
    };

    (
        tezos_env.main_chain_id().expect("invalid chain id"),
        tezos_env.genesis_header_hash().expect("invalid genesis header hash"),
        genesis_context_hash,
    )
}

/// Empty message
#[derive(Serialize, Deserialize, Debug)]
struct NoopMessage;

fn create_logger() -> Logger {
    let drain = slog_async::Async::new(slog_term::FullFormat::new(slog_term::TermDecorator::new().build()).build().fuse()).build().filter_level(Level::Info).fuse();

    Logger::root(drain, slog::o!())
}

mod common {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};

    pub fn prepare_empty_dir(dir_name: &str) -> String {
        let path = test_storage_dir_path(dir_name);
        if path.exists() {
            fs::remove_dir_all(&path).unwrap_or_else(|_| panic!("Failed to delete directory: {:?}", &path));
        }
        fs::create_dir_all(&path).unwrap_or_else(|_| panic!("Failed to create directory: {:?}", &path));
        String::from(path.to_str().unwrap())
    }

    pub fn test_storage_dir_path(dir_name: &str) -> PathBuf {
        let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined");
        let path = Path::new(out_dir.as_str())
            .join(Path::new(dir_name))
            .to_path_buf();
        path
    }

    pub fn is_ocaml_log_enabled() -> bool {
        env::var("OCAML_LOG_ENABLED")
            .unwrap_or("false".to_string())
            .parse::<bool>().unwrap()
    }

    pub fn no_of_ffi_calls_treshold_for_gc() -> i32 {
        env::var("OCAML_CALLS_GC")
            .unwrap_or("2000".to_string())
            .parse::<i32>().unwrap()
    }
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