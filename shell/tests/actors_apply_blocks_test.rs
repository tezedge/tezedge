// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]
extern crate test;

/// Big integration test for actors, covers two main use cases:
/// 1. test_scenario_for_apply_blocks_with_chain_feeder_and_check_context - see fn description
/// 2. test_scenario_for_add_operations_to_mempool_and_check_state - see fn description

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::mpsc::{channel, Receiver as QueueReceiver};
use std::time::{Duration, Instant, SystemTime};

use riker::actors::*;
use slog::{info, Logger};

use crypto::hash::{BlockHash, HashType, OperationHash};
use shell::shell_channel::{CurrentMempoolState, MempoolOperationReceived, ShellChannelRef, ShellChannelTopic};
use storage::{BlockHeaderWithHash, BlockMetaStorage, BlockStorage, BlockStorageReader, ChainMetaStorage, context_key, MempoolStorage, OperationsMetaStorage, OperationsStorage};
use storage::chain_meta_storage::ChainMetaStorageReader;
use storage::context::{ContextApi, TezedgeContext};
use storage::mempool_storage::MempoolOperationType;
use storage::persistent::PersistentStorage;
use storage::tests_common::TmpStorage;
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_messages::p2p::binary_message::MessageHash;
use tezos_messages::p2p::encoding::operations_for_blocks::OperationsForBlocksMessage;
use tezos_messages::p2p::encoding::prelude::Operation;

use crate::samples::OperationsForBlocksMessageKey;

mod common;
mod samples;

#[ignore]
#[test]
fn test_actors_apply_blocks_and_check_context_and_mempool() -> Result<(), failure::Error> {
    // logger
    let log_level = common::log_level();
    let log = common::create_logger(log_level);

    // prepare data - we have stored 1326 request, apply just 1324, and 1325,1326 will be used for mempool test
    let (requests, operations, tezos_env) = samples::read_data_apply_block_request_until_1326();

    // start node
    let node = common::infra::NodeInfrastructure::start(
        TmpStorage::create(common::prepare_empty_dir("__test_actors_apply_blocks_and_check_context_and_mempool"))?,
        &common::prepare_empty_dir("__test_actors_apply_blocks_and_check_context_and_mempool_context"),
        "test_actors_apply_blocks_and_check_context_and_mempool",
        &tezos_env,
        None,
        None,
        tezos_identity::Identity::generate(0f64),
        (log, log_level),
    )?;

    // test mempool state for validation
    let (test_result_sender, test_result_receiver) = channel();
    let _ = test_actor::TestActor::actor(&node.actor_system, node.shell_channel.clone(), test_result_sender);

    let clocks = Instant::now();

    // 1. test - apply and context - prepare data for apply blocks and wait for current head, and check context
    assert!(
        test_scenario_for_apply_blocks_with_chain_feeder_and_check_context(
            &node.tmp_storage.storage(),
            &node.tezos_env,
            node.log.clone(),
            &requests,
            &operations,
            1324,
        ).is_ok()
    );

    // 2. test - mempool test
    assert!(
        test_scenario_for_add_operations_to_mempool_and_check_state(
            test_result_receiver,
            node.shell_channel.clone(),
            &node.tmp_storage.storage(),
            &requests[1323],
            &requests[1324],
            &requests[1325],
        ).is_ok()
    );

    println!("\nDone in {:?}!", clocks.elapsed());

    drop(node);

    Ok(())
}

fn check_context(persistent_storage: &PersistentStorage) -> Result<(), failure::Error> {
    let context = TezedgeContext::new(
        BlockStorage::new(&persistent_storage),
        persistent_storage.merkle(),
    );

    // check level 0
    let ctx_hash = context.level_to_hash(0)?;
    if let Some(data) = context.get_key_from_history(&ctx_hash, &context_key!("protocol"))? {
        assert_eq!("PtYuensgYBb3G3x1hLLbCmcav8ue8Kyd2khADcL5LsT5R1hcXex", HashType::ProtocolHash.bytes_to_string(&data));
    } else {
        panic!(format!("Protocol not found in context for level: {}", 0));
    }

    // check level 1
    let ctx_hash = context.level_to_hash(1)?;
    if let Some(data) = context.get_key_from_history(&ctx_hash, &context_key!("protocol"))? {
        assert_eq!("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS", HashType::ProtocolHash.bytes_to_string(&data));
    } else {
        panic!(format!("Protocol not found in context for level: {}", 1));
    }

    // check level 2
    let ctx_hash = context.level_to_hash(2)?;
    if let Some(data) = context.get_key_from_history(&ctx_hash, &context_key!("protocol"))? {
        assert_eq!("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS", HashType::ProtocolHash.bytes_to_string(&data));
    } else {
        panic!(format!("Protocol not found in context for level: {}", 2));
    }

    // check level 1324 with merkle storage
    let m = persistent_storage.merkle();
    let merkle = m.write().unwrap();
    // get final hash from last commit in merkle storage
    let merkle_last_hash = merkle.get_last_commit_hash();

    // compare with context hash of last applied (1324th) block
    let bhwithhash = BlockStorage::new(&persistent_storage).get_by_block_level(1324);
    let hash = bhwithhash.unwrap().unwrap();
    let ctx_hash = hash.header.context();

    assert_eq!(*ctx_hash, merkle_last_hash.unwrap());
    let stats = merkle.get_merkle_stats().unwrap();
    println!("Avg set exec time in ns: {}", stats.perf_stats.avg_set_exec_time_ns);

    Ok(())
}

/// Test scenario applies all requests to the apply_to_level,
/// then waits for context_listener to commit context,
/// and then validates stored context to dedicated context exported from ocaml on the same level
fn test_scenario_for_apply_blocks_with_chain_feeder_and_check_context(
    persistent_storage: &PersistentStorage,
    tezos_env: &TezosEnvironmentConfiguration,
    log: Logger,
    requests: &Vec<String>,
    operations: &HashMap<OperationsForBlocksMessageKey, OperationsForBlocksMessage>,
    apply_to_level: i32) -> Result<(), failure::Error> {
    // prepare dbs
    let block_storage = BlockStorage::new(&persistent_storage);
    let block_meta_storage = BlockMetaStorage::new(&persistent_storage);
    let chain_meta_storage = ChainMetaStorage::new(&persistent_storage);
    let operations_storage = OperationsStorage::new(&persistent_storage);
    let operations_meta_storage = OperationsMetaStorage::new(&persistent_storage);

    let chain_id = tezos_env.main_chain_id().expect("invalid chain id");

    let clocks = Instant::now();

    // let's insert stored requests to database
    for request in requests {

        // parse request
        let request = samples::from_captured_bytes(request)?;
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
        let validation_pass: u8 = block.header.validation_pass();
        for vp in 0..validation_pass {
            if let Some(msg) = operations.get(&OperationsForBlocksMessageKey::new(block.hash.clone(), vp as i8)) {
                operations_storage.put_operations(msg)?;
                let _ = operations_meta_storage.put_operations(msg)?;
            }
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
        match chain_meta_storage.get_current_head(&chain_id)? {
            None => (),
            Some(head) => {
                if head.level() >= &apply_to_level {
                    let header = block_storage.get(head.block_hash()).expect("failed to read current head").expect("current head not found");
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
    test_result_receiver: QueueReceiver<Arc<CurrentMempoolState>>,
    shell_channel: ShellChannelRef,
    persistent_storage: &PersistentStorage,
    last_applied_request_1324: &String,
    request_1325: &String,
    request_1326: &String) -> Result<(), failure::Error> {
    let last_applied_block: BlockHash = samples::from_captured_bytes(last_applied_request_1324)?.block_header.message_hash()?;
    let mut mempool_storage = MempoolStorage::new(&persistent_storage);

    // wait mempool for last_applied_block
    let mut current_mempool_state: Option<Arc<CurrentMempoolState>> = None;
    while let Ok(result) = test_result_receiver.recv_timeout(Duration::from_secs(10)) {
        let done = if let Some(head) = &result.head {
            *head == last_applied_block
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
    let mempool_head = current_mempool_state.head.as_ref().unwrap();
    assert_eq!(*mempool_head, last_applied_block);

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
    let mut current_mempool_state: Option<Arc<CurrentMempoolState>> = None;
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
    let mut current_mempool_state: Option<Arc<CurrentMempoolState>> = None;
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
    let request = samples::from_captured_bytes(request)?;
    let mut operation_hashes = HashSet::new();
    for operations in request.operations {
        for operation in operations {
            // this is done by chain_manager when received new operations

            let operation_hash = operation.message_hash()?;

            // add to mempool storage
            mempool_storage.put(
                MempoolOperationType::Pending,
                operation.into(),
                SystemTime::now(),
            )?;


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

mod test_actor {
    use std::sync::{Arc, Mutex};
    use std::sync::mpsc::Sender as QueueSender;

    use riker::actors::*;
    use slog::{debug, warn};

    use shell::shell_channel::{CurrentMempoolState, ShellChannelMsg, ShellChannelRef, ShellChannelTopic};

    #[actor(ShellChannelMsg)]
    pub(crate) struct TestActor {
        result_sender: Arc<Mutex<QueueSender<Arc<CurrentMempoolState>>>>,
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

    impl ActorFactoryArgs<(ShellChannelRef, Arc<Mutex<QueueSender<Arc<CurrentMempoolState>>>>)> for TestActor {
        fn create_args((shell_channel, result_sender): (ShellChannelRef, Arc<Mutex<QueueSender<Arc<CurrentMempoolState>>>>)) -> Self {
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

        pub fn actor(sys: &ActorSystem, shell_channel: ShellChannelRef, result_sender: QueueSender<Arc<CurrentMempoolState>>) -> Result<TestActorRef, CreateError> {
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
                    self.result_sender.lock().unwrap().send(Arc::new(new_mempool_state.read().unwrap().clone()))?;
                }
                _ => ()
            }

            Ok(())
        }
    }
}
