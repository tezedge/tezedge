// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]
extern crate test;

/// Big integration test for actors, covers two main use cases:
/// 1. test_scenario_for_apply_blocks_with_chain_feeder_and_check_context - see fn description
/// 2. test_scenario_for_add_operations_to_mempool_and_check_state - see fn description

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver as QueueReceiver};
use std::thread;
use std::time::{Duration, SystemTime, Instant};

use jsonpath::Selector;
use riker::actors::*;
use riker::system::SystemBuilder;
use serde_json::Value;
use slog::{info, Logger};

use crypto::hash::{HashType, OperationHash};
use shell::chain_feeder::ChainFeeder;
use shell::context_listener::ContextListener;
use shell::mempool_prevalidator::MempoolPrevalidator;
use shell::shell_channel::{CurrentMempoolState, MempoolOperationReceived, ShellChannel, ShellChannelRef, ShellChannelTopic, ShuttingDown};
use storage::{BlockHeaderWithHash, BlockMetaStorage, BlockMetaStorageReader, BlockStorage, BlockStorageReader, MempoolStorage, OperationsMetaStorage, OperationsStorage, resolve_storage_init_chain_data};
use storage::context::{ContextApi, ContextIndex, TezedgeContext};
use storage::mempool_storage::MempoolOperationType;
use storage::persistent::{ContextList, PersistentStorage};
use storage::skip_list::Bucket;
use storage::tests_common::TmpStorage;
use tezos_api::environment::{TEZOS_ENV, TezosEnvironmentConfiguration};
use tezos_api::ffi::{ApplyBlockRequest, FfiMessage, RustBytes, TezosRuntimeConfiguration};
use tezos_messages::p2p::binary_message::MessageHash;
use tezos_messages::p2p::encoding::operation::OperationMessage;
use tezos_messages::p2p::encoding::operations_for_blocks::{OperationsForBlock, OperationsForBlocksMessage};
use tezos_messages::p2p::encoding::operations_for_blocks;
use tezos_messages::p2p::encoding::prelude::Operation;
use tezos_wrapper::{TezosApiConnectionPool, TezosApiConnectionPoolConfiguration};
use tezos_wrapper::service::{ExecutableProtocolRunner, ProtocolEndpointConfiguration, ProtocolRunnerEndpoint};

mod common;

#[ignore]
#[test]
fn test_actors_apply_blocks_and_check_context_and_mempool() -> Result<(), failure::Error> {

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

    let storage_db_path = PathBuf::from(storage_db_path);
    let context_db_path = PathBuf::from(context_db_path);
    let init_storage_data = resolve_storage_init_chain_data(&tezos_env, &storage_db_path, &context_db_path, &None, &log)
        .expect("Failed to resolve init storage chain data");

    // apply block protocol runner endpoint
    let apply_protocol_runner = common::protocol_runner_executable_path();
    let mut apply_protocol_runner_endpoint = ProtocolRunnerEndpoint::<ExecutableProtocolRunner>::new(
        "test_protocol_runner_endpoint",
        ProtocolEndpointConfiguration::new(
            TezosRuntimeConfiguration {
                log_enabled: common::is_ocaml_log_enabled(),
                no_of_ffi_calls_treshold_for_gc: common::no_of_ffi_calls_treshold_for_gc(),
                debug_mode: false,
            },
            tezos_env.clone(),
            false,
            &context_db_path,
            &apply_protocol_runner,
            log_level.clone(),
            true,
        ),
        log.clone(),
    );
    let (apply_restarting_feature, apply_protocol_commands, apply_protocol_events) = match apply_protocol_runner_endpoint.start_in_restarting_mode() {
        Ok(restarting_feature) => {
            let ProtocolRunnerEndpoint {
                commands,
                events,
                ..
            } = apply_protocol_runner_endpoint;
            (restarting_feature, commands, events)
        }
        Err(e) => panic!("Error to start test_protocol_runner_endpoint: {} - error: {:?}", apply_protocol_runner.as_os_str().to_str().unwrap_or("-none-"), e)
    };

    // create pool for ffi protocol runner connections (used just for readonly context)
    let tezos_readonly_api = Arc::new(
        TezosApiConnectionPool::new_with_readonly_context(
            String::from("test_ tezos_readonly_api_pool"),
            TezosApiConnectionPoolConfiguration {
                min_connections: 0,
                max_connections: 2,
                connection_timeout: Duration::from_secs(3),
                max_lifetime: Duration::from_secs(60),
                idle_timeout: Duration::from_secs(60),
            },
            ProtocolEndpointConfiguration::new(
                TezosRuntimeConfiguration {
                    log_enabled: common::is_ocaml_log_enabled(),
                    no_of_ffi_calls_treshold_for_gc: common::no_of_ffi_calls_treshold_for_gc(),
                    debug_mode: false,
                },
                tezos_env.clone(),
                false,
                &context_db_path,
                &common::protocol_runner_executable_path(),
                log_level,
                false,
            ),
            log.clone(),
        )
    );

    // test mempool state for validation
    let (test_result_sender, test_result_receiver) = channel();

    // run actor's
    let actor_system = SystemBuilder::new().name("test_actors_apply_blocks_and_check_context").log(log.clone()).create().expect("Failed to create actor system");
    let shell_channel = ShellChannel::actor(&actor_system).expect("Failed to create shell channel");
    let _ = test_actor::TestActor::actor(&actor_system, shell_channel.clone(), test_result_sender);
    let _ = ContextListener::actor(&actor_system, &persistent_storage, apply_protocol_events.expect("Context listener needs event server"), log.clone(), false).expect("Failed to create context event listener");
    let _ = ChainFeeder::actor(&actor_system, shell_channel.clone(), &persistent_storage, &init_storage_data, &tezos_env, apply_protocol_commands, log.clone()).expect("Failed to create chain feeder");
    let _ = MempoolPrevalidator::actor(
        &actor_system,
        shell_channel.clone(),
        &persistent_storage,
        &init_storage_data,
        tezos_readonly_api.clone(),
        log.clone(),
    ).expect("Failed to create chain feeder");

    // we have stored 1326 request, apply just 1324, and 1325,1326 will be used for mempool test
    let requests = test_data::read_apply_block_requests_until_1326();


    let clocks = Instant::now();

    // 1. test - apply and context - prepare data for apply blocks and wait for current head, and check context
    assert!(
        test_scenario_for_apply_blocks_with_chain_feeder_and_check_context(
            &persistent_storage,
            tezos_env,
            log.clone(),
            &requests,
            1324,
        ).is_ok()
    );

    // 2. test - mempool test
    assert!(
        test_scenario_for_add_operations_to_mempool_and_check_state(
            test_result_receiver,
            shell_channel.clone(),
            &persistent_storage,
            &requests[1324],
            &requests[1325],
        ).is_ok()
    );

    let clocks = clocks.elapsed();
    println!("\nDone in {:?}!", clocks);

    // clean up
    // shutdown events listening
    apply_restarting_feature.store(false, Ordering::Release);

    thread::sleep(Duration::from_secs(3));
    shell_channel.tell(
        Publish {
            msg: ShuttingDown.into(),
            topic: ShellChannelTopic::ShellCommands.into(),
        }, None,
    );
    thread::sleep(Duration::from_secs(2));

    let _ = actor_system.shutdown();

    Ok(())
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

fn assert_ctxt(ctxt: BTreeMap<String, Bucket<Vec<u8>>>, ocaml_ctxt_as_json: String) {
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

/// Test scenario applies all requests to the apply_to_level,
/// then waits for context_listener to commit context,
/// and then validates stored context to dedicated context exported from ocaml on the same level
fn test_scenario_for_apply_blocks_with_chain_feeder_and_check_context(
    persistent_storage: &PersistentStorage,
    tezos_env: &TezosEnvironmentConfiguration,
    log: Logger,
    requests: &Vec<String>,
    apply_to_level: i32) -> Result<(), failure::Error> {
    // prepare dbs
    let mut block_storage = BlockStorage::new(&persistent_storage);
    let mut block_meta_storage = BlockMetaStorage::new(&persistent_storage);
    let mut operations_storage = OperationsStorage::new(&persistent_storage);
    let mut operations_meta_storage = OperationsMetaStorage::new(&persistent_storage);

    let chain_id = tezos_env.main_chain_id().expect("invalid chain id");

    let clocks = Instant::now();

    // let's insert stored requests to database
    for request in requests {

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
        block_meta_storage.put_block_header(&block, &chain_id, &log)?;
        operations_meta_storage.put_block_header(&block, &chain_id)?;

        // store operations to db
        for (idx, ops) in request.operations.into_iter().enumerate() {
            let opb = OperationsForBlock::new(block.hash.clone(), idx as i8);
            let msg: OperationsForBlocksMessage = OperationsForBlocksMessage::new(opb, operations_for_blocks::Path::Op, ops);
            operations_storage.put_operations(&msg)?;
            operations_meta_storage.put_operations(&msg)?;
        }
        assert!(operations_meta_storage.is_complete(&block.hash)?);

        if block.header.level() >= apply_to_level {
            break;
        }
    }

    let clocks = clocks.elapsed();
    println!("\n[Insert] done in {:?}!", clocks);

    // wait context_listener to finished context for applied blocks
    info!(log, "Waiting for context processing"; "level" => apply_to_level);
    loop {
        match block_meta_storage.load_current_head()? {
            None => (),
            Some((h, _)) => {
                let meta = block_meta_storage.get(&h)?;
                if let Some(m) = meta {
                    let header = block_storage.get(&h).expect("failed to read current head").expect("current head not found");
                    if m.level() >= apply_to_level {
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
    info!(log, "Context done and successfully applied to level"; "level" => apply_to_level);

    // check context
    check_context(&persistent_storage)
}

/// Starts on mempool current state for last applied block, which is supossed to be 1324.
/// Mempool for 1324 is checked, and then operations from 1325 are stored to mempool for validation,
/// Test waits for all 6 result and than checks current mempool state, if contains `applied` 6 operations.
///
/// Than tries to validate operations from block 1326, which are `branch_delayed`.
fn test_scenario_for_add_operations_to_mempool_and_check_state(
    test_result_receiver: QueueReceiver<CurrentMempoolState>,
    shell_channel: ShellChannelRef,
    persistent_storage: &PersistentStorage,
    request_1325: &String,
    request_1326: &String) -> Result<(), failure::Error> {
    let last_applied_level = 1324;
    let mut mempool_storage = MempoolStorage::new(&persistent_storage);

    // wait mempool for last_applied_level
    let mut current_mempool_state: Option<CurrentMempoolState> = None;
    while let Ok(result) = test_result_receiver.recv_timeout(Duration::from_secs(10)) {
        let done = if let Some(head) = &result.head {
            head.level == last_applied_level
        } else {
            false
        };
        current_mempool_state = Some(result);

        if done {
            break;
        }
    }

    // check current mempool state, should be on last applied block 1324
    assert!(current_mempool_state.is_some());
    let current_mempool_state = current_mempool_state.unwrap();
    assert!(current_mempool_state.head.is_some());
    let mempool_head = current_mempool_state.head.unwrap();
    assert_eq!(mempool_head.level, last_applied_level);

    // check operations in mempool - should by empty all
    assert!(current_mempool_state.result.applied.is_empty());
    assert!(current_mempool_state.result.branch_delayed.is_empty());
    assert!(current_mempool_state.result.branch_refused.is_empty());
    assert!(current_mempool_state.result.refused.is_empty());

    // add operations from 1325 to mempool - should by applied
    let operations_from_1325 = add_operations_to_mempool(request_1325, shell_channel.clone(), &mut mempool_storage)?;
    let operations_from_1325_count = operations_from_1325.len();
    assert_ne!(0, operations_from_1325_count);

    // we expect here message for every operation
    let mut current_mempool_state: Option<CurrentMempoolState> = None;
    while let Ok(result) = test_result_receiver.recv_timeout(Duration::from_secs(10)) {
        let done = contains_all_keys(&result.operations, &operations_from_1325);
        current_mempool_state = Some(result);
        if done {
            break;
        }
    }

    // check mempool current state after operations 1325
    assert!(current_mempool_state.is_some());
    let current_mempool_state = current_mempool_state.unwrap();
    assert_eq!(operations_from_1325_count, current_mempool_state.result.applied.len());
    assert!(current_mempool_state.result.branch_delayed.is_empty());
    assert!(current_mempool_state.result.branch_refused.is_empty());
    assert!(current_mempool_state.result.refused.is_empty());

    // add operations from 1326 to mempool - should by branch_delay
    let operations_from_1326 = add_operations_to_mempool(request_1326, shell_channel.clone(), &mut mempool_storage)?;
    let operations_from_1326_count = operations_from_1326.len();
    assert_ne!(0, operations_from_1326_count);

    // we expect here message for every operation
    let mut current_mempool_state: Option<CurrentMempoolState> = None;
    while let Ok(result) = test_result_receiver.recv_timeout(Duration::from_secs(10)) {
        let done = contains_all_keys(&result.operations, &operations_from_1326);
        current_mempool_state = Some(result);
        if done {
            break;
        }
    }

    // check mempool current state after operations 1326
    assert!(current_mempool_state.is_some());
    let current_mempool_state = current_mempool_state.unwrap();
    assert_eq!(operations_from_1325_count, current_mempool_state.result.applied.len());
    assert_eq!(operations_from_1326_count, current_mempool_state.result.branch_delayed.len());
    assert!(current_mempool_state.result.branch_refused.is_empty());
    assert!(current_mempool_state.result.refused.is_empty());

    Ok(())
}

fn add_operations_to_mempool(request: &String, shell_channel: ShellChannelRef, mempool_storage: &mut MempoolStorage) -> Result<HashSet<OperationHash>, failure::Error> {
    let request: RustBytes = hex::decode(request)?;
    let request = ApplyBlockRequest::from_rust_bytes(request)?;
    let mut operation_hashes = HashSet::new();
    for operations in request.operations {
        for operation in operations {
            // this is done by chain_manager when received new operations

            // add to mempool storage
            mempool_storage.put(
                MempoolOperationType::Pending,
                OperationMessage::new(operation.clone()),
                SystemTime::now(),
            )?;

            let operation_hash = operation.message_hash()?;

            // ping channel - mempool_prevalidator listens
            shell_channel.tell(
                Publish {
                    msg: MempoolOperationReceived {
                        operation_hash: operation_hash.clone(),
                        operation_type: MempoolOperationType::Pending,
                    }.into(),
                    topic: ShellChannelTopic::ShellEvents.into(),
                },
                None,
            );

            operation_hashes.insert(operation_hash);
        }
    }

    Ok(operation_hashes)
}

fn contains_all_keys(map: &HashMap<OperationHash, Operation>, keys: &HashSet<OperationHash>) -> bool {
    let mut contains_counter = 0;
    for key in keys {
        if map.contains_key(key) {
            contains_counter += 1;
        }
    }
    contains_counter == keys.len()
}

mod test_data {
    use std::{env, io};
    use std::fs::File;
    use std::path::Path;

    use tezos_api::environment::TezosEnvironment;

    pub const TEZOS_NETWORK: TezosEnvironment = TezosEnvironment::Carthagenet;

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

mod test_actor {
    use std::sync::{Arc, Mutex};
    use std::sync::mpsc::Sender as QueueSender;

    use riker::actors::*;
    use slog::{debug, warn};

    use shell::shell_channel::{CurrentMempoolState, ShellChannelMsg, ShellChannelRef, ShellChannelTopic};

    #[actor(ShellChannelMsg)]
    pub(crate) struct TestActor {
        result_sender: Arc<Mutex<QueueSender<CurrentMempoolState>>>,
        shell_channel: ShellChannelRef,
    }

    pub type TestActorRef = ActorRef<TestActorMsg>;

    impl Actor for TestActor {
        type Msg = TestActorMsg;

        fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
            self.shell_channel.tell(Subscribe {
                actor: Box::new(ctx.myself()),
                topic: ShellChannelTopic::ShellEvents.into(),
            }, ctx.myself().into());
        }

        fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<BasicActorRef>) {
            self.receive(ctx, msg, sender);
        }
    }

    impl ActorFactoryArgs<(ShellChannelRef, Arc<Mutex<QueueSender<CurrentMempoolState>>>)> for TestActor {
        fn create_args((shell_channel, result_sender): (ShellChannelRef, Arc<Mutex<QueueSender<CurrentMempoolState>>>)) -> Self {
            Self {
                shell_channel,
                result_sender,
            }
        }
    }

    impl Receive<ShellChannelMsg> for TestActor {
        type Msg = TestActorMsg;

        fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ShellChannelMsg, _sender: Sender) {
            match self.process_shell_channel_message(ctx, msg) {
                Ok(_) => (),
                Err(e) => warn!(ctx.system.log(), "Failed to process shell channel message"; "reason" => format!("{:?}", e)),
            }
        }
    }

    impl TestActor {
        pub fn name() -> &'static str { "test-actor" }

        pub fn actor(sys: &ActorSystem, shell_channel: ShellChannelRef, result_sender: QueueSender<CurrentMempoolState>) -> Result<TestActorRef, CreateError> {
            Ok(
                sys.actor_of_props::<TestActor>(
                    Self::name(),
                    Props::new_args((shell_channel, Arc::new(Mutex::new(result_sender)))),
                )?
            )
        }

        fn process_shell_channel_message(&mut self, ctx: &Context<TestActorMsg>, msg: ShellChannelMsg) -> Result<(), failure::Error> {
            match msg {
                ShellChannelMsg::MempoolStateChanged(new_mempool_state) => {
                    debug!(ctx.system.log(), "TestActor received event"; "mempool" => format!("{:?}", new_mempool_state));
                    self.result_sender.lock().unwrap().send(new_mempool_state)?;
                }
                _ => ()
            }

            Ok(())
        }
    }
}