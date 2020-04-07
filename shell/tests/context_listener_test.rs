// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]
extern crate test;

use std::sync::Arc;
use std::thread;

use riker::system::SystemBuilder;
use serde::{Deserialize, Serialize};
use slog::{Drain, Level, Logger, warn};

use crypto::hash::HashType;
use shell::context_listener::ContextListener;
use storage::{BlockHeaderWithHash, BlockStorage};
use storage::context::{ContextApi, ContextIndex, TezedgeContext};
use storage::skip_list::Bucket;
use storage::tests_common::TmpStorage;
use tezos_api::client::TezosStorageInitInfo;
use tezos_api::ffi::TezosRuntimeConfiguration;
use tezos_client::client;
use tezos_context::channel::*;
use tezos_interop::ffi;
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::{BlockHeader, BlockHeaderBuilder};
use tezos_wrapper::service::IpcEvtServer;

#[test]
fn test_apply_first_three_block_and_check_context() -> Result<(), failure::Error> {

    // logger
    let log = create_logger();

    // storage
    let tmp_storage = TmpStorage::create(common::prepare_empty_dir(&format!("__shell_test_apply_blocks{:?}_storage", "TODO_123")))?;
    let persistent_storage = tmp_storage.storage();

    // init storage with genesis (important step)
    let mut block_storage = BlockStorage::new(&persistent_storage);
    // TODO: TE-98 - refactor - according to patch_context.ml / state.ml module Locked_block =
    // TODO: nasetovat realne hodnoty - operations_hash a context.zero a proto_data
    let genesis = BlockHeaderWithHash {
        hash: HashType::BlockHash.string_to_bytes("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe")?,
        header: Arc::new(
            BlockHeaderBuilder::default()
                .level(0)
                .proto(0)
                .predecessor(HashType::BlockHash.string_to_bytes("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe")?)
                .timestamp(5_635_634)
                .validation_pass(0)
                .operations_hash(HashType::OperationListListHash.string_to_bytes("LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc")?)
                .fitness(vec![])
                .context(HashType::ContextHash.string_to_bytes("CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd")?)
                .protocol_data(vec![])
                .build().unwrap()
        ),
    };
    block_storage.put_block_header(&genesis)?;

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
    let actor_system = SystemBuilder::new().name("test_apply_first_three_block_and_check_context").log(log.clone()).create().expect("Failed to create actor system");
    let _ = ContextListener::actor(&actor_system, &persistent_storage, event_server, log.clone()).expect("Failed to create context event listener");

    // run apply blocks
    let _ = apply_first_three_blocks(&mut block_storage)?;
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
        assert_eq!("Ps6mwMrF2ER2s51cp9yYpjDcuzQjsc2yAz8bQsRgdaRxw4Fk95H", HashType::ProtocolHash.bytes_to_string(&data));
    } else {
        panic!(format!("Protocol not found in context for level: {}", 0));
    }

    // check level 1
    if let Some(Bucket::Exists(data)) = context.get_key(&ContextIndex::new(Some(1), None), &vec!["protocol".to_string()])? {
        assert_eq!("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP", HashType::ProtocolHash.bytes_to_string(&data));
    } else {
        panic!(format!("Protocol not found in context for level: {}", 1));
    }

    // check level 2
    if let Some(Bucket::Exists(data)) = context.get_key(&ContextIndex::new(Some(2), None), &vec!["protocol".to_string()])? {
        assert_eq!("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP", HashType::ProtocolHash.bytes_to_string(&data));
    } else {
        panic!(format!("Protocol not found in context for level: {}", 2));
    }

    Ok(())
}

fn apply_first_three_blocks(block_storage: &mut BlockStorage) -> Result<(), failure::Error> {
    ffi::change_runtime_configuration(
        TezosRuntimeConfiguration {
            log_enabled: common::is_ocaml_log_enabled(),
            no_of_ffi_calls_treshold_for_gc: common::no_of_ffi_calls_treshold_for_gc(),
        }
    ).unwrap().unwrap();

    // init empty storage for test (not measuring)
    let TezosStorageInitInfo { chain_id, .. } = client::init_storage(
        common::prepare_empty_dir(&format!("__shell_test_apply_blocks{:?}", "TODO_123")),
        test_data::TEZOS_ENV,
        false,
    ).unwrap();

    // store and apply first block - level 1
    let block = BlockHeaderWithHash {
        hash: HashType::BlockHash.string_to_bytes(&HashType::BlockHash.bytes_to_string(&hex::decode(test_data::BLOCK_HEADER_HASH_LEVEL_1)?))?,
        header: Arc::new(BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap())?),
    };
    block_storage.put_block_header(&block)?;

    let apply_block_result = client::apply_block(
        &chain_id,
        &block.header,
        &test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_1,
            test_data::block_header_level1_operations(),
        ),
    );
    assert_eq!(test_data::context_hash(test_data::BLOCK_HEADER_LEVEL_1_CONTEXT_HASH), apply_block_result?.context_hash);

    // store and apply second block - level 2
    let block = BlockHeaderWithHash {
        hash: HashType::BlockHash.string_to_bytes(&HashType::BlockHash.bytes_to_string(&hex::decode(test_data::BLOCK_HEADER_HASH_LEVEL_2)?))?,
        header: Arc::new(BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_2).unwrap())?),
    };
    block_storage.put_block_header(&block)?;

    let apply_block_result = client::apply_block(
        &chain_id,
        &block.header,
        &test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_2,
            test_data::block_header_level2_operations(),
        ),
    );
    assert_eq!("lvl 2, fit 2, prio 5, 0 ops", &apply_block_result?.validation_result_message);

    // store apply third block - level 3
    let block = BlockHeaderWithHash {
        hash: HashType::BlockHash.string_to_bytes(&HashType::BlockHash.bytes_to_string(&hex::decode(test_data::BLOCK_HEADER_HASH_LEVEL_3)?))?,
        header: Arc::new(BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_3).unwrap())?),
    };
    block_storage.put_block_header(&block)?;

    let apply_block_result = client::apply_block(
        &chain_id,
        &block.header,
        &test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_3,
            test_data::block_header_level3_operations(),
        ),
    );
    Ok(assert_eq!("lvl 3, fit 5, prio 12, 1 ops", &apply_block_result?.validation_result_message))
}

/// Empty message
#[derive(Serialize, Deserialize, Debug)]
struct NoopMessage;

fn create_logger() -> Logger {
    let drain = slog_async::Async::new(slog_term::FullFormat::new(slog_term::TermDecorator::new().build()).build().fuse()).build().filter_level(Level::Info).fuse();

    Logger::root(drain, slog::o!())
}

mod test_data {
    use crypto::hash::{ContextHash, HashType};
    use tezos_api::environment::TezosEnvironment;
    use tezos_messages::p2p::binary_message::BinaryMessage;
    use tezos_messages::p2p::encoding::prelude::*;

    pub const TEZOS_ENV: TezosEnvironment = TezosEnvironment::Alphanet;

    pub fn context_hash(hash: &str) -> ContextHash {
        HashType::ContextHash
            .string_to_bytes(hash)
            .unwrap()
    }

    // BMPtRJqFGQJRTfn8bXQR2grLE1M97XnUmG5vgjHMW7St1Wub7Cd
    pub const BLOCK_HEADER_HASH_LEVEL_1: &str = "dd9fb5edc4f29e7d28f41fe56d57ad172b7686ed140ad50294488b68de29474d";
    pub const BLOCK_HEADER_LEVEL_1: &str = include_str!("../../tezos/client/tests/resources/block_header_level1.bytes");
    pub const BLOCK_HEADER_LEVEL_1_CONTEXT_HASH: &str = "CoV16kW8WgL51SpcftQKdeqc94D6ekghMgPMmEn7TSZzFA697PeE";

    pub fn block_header_level1_operations() -> Vec<Vec<String>> {
        vec![]
    }

    // BLwKksYwrxt39exDei7yi47h7aMcVY2kZMZhTwEEoSUwToQUiDV
    pub const BLOCK_HEADER_HASH_LEVEL_2: &str = "a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627";
    pub const BLOCK_HEADER_LEVEL_2: &str = "0000000201dd9fb5edc4f29e7d28f41fe56d57ad172b7686ed140ad50294488b68de29474d000000005c017cd804683625c2445a4e9564bf710c5528fd99a7d150d2a2a323bc22ff9e2710da4f6d0000001100000001000000000800000000000000029bd8c75dec93c276d2d8e8febc3aa6c9471cb2cb42236b3ab4ca5f1f2a0892f6000500000003ba671eef00d6a8bea20a4677fae51268ab6be7bd8cfc373cd6ac9e0a00064efcc404e1fb39409c5df255f7651e3d1bb5d91cb2172b687e5d56ebde58cfd92e1855aaafbf05";

    pub fn block_header_level2_operations() -> Vec<Vec<String>> {
        vec![
            vec![],
            vec![],
            vec![],
            vec![]
        ]
    }

    // BLTQ5B4T4Tyzqfm3Yfwi26WmdQScr6UXVSE9du6N71LYjgSwbtc
    pub const BLOCK_HEADER_HASH_LEVEL_3: &str = "61e687e852460b28f0f9540ccecf8f6cf87a5ad472c814612f0179caf4b9f673";
    pub const BLOCK_HEADER_LEVEL_3: &str = "0000000301a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000005c017ed604dfcb6b41e91650bb908618b2740a6167d9072c3230e388b24feeef04c98dc27f000000110000000100000000080000000000000005f06879947f3d9959090f27054062ed23dbf9f7bd4b3c8a6e86008daabb07913e000c00000003e5445371002b9745d767d7f164a39e7f373a0f25166794cba491010ab92b0e281b570057efc78120758ff26a33301870f361d780594911549bcb7debbacd8a142e0b76a605";

    pub fn block_header_level3_operations() -> Vec<Vec<String>> {
        vec![
            vec!["a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000000236663bacdca76094fdb73150092659d463fec94eda44ba4db10973a1ad057ef53a5b3239a1b9c383af803fc275465bd28057d68f3cab46adfd5b2452e863ff0a".to_string()],
            vec![],
            vec![],
            vec![]
        ]
    }

    pub fn block_operations_from_hex(block_hash: &str, hex_operations: Vec<Vec<String>>) -> Vec<Option<OperationsForBlocksMessage>> {
        hex_operations
            .into_iter()
            .map(|bo| {
                let ops = bo
                    .into_iter()
                    .map(|op| Operation::from_bytes(hex::decode(op).unwrap()).unwrap())
                    .collect();
                Some(OperationsForBlocksMessage::new(OperationsForBlock::new(hex::decode(block_hash).unwrap(), 4), Path::Op, ops))
            })
            .collect()
    }
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