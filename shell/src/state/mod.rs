// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::{Arc, PoisonError};

use thiserror::Error;

use crypto::hash::BlockHash;
use storage::StorageError;
use tezos_messages::p2p::encoding::prelude::OperationsForBlock;

pub mod bootstrap_state;
pub mod chain_state;
pub mod data_requester;
pub mod head_state;
pub mod peer_state;
pub mod synchronization_state;

/// Possible errors for state processing
#[derive(Debug, Error)]
pub enum StateError {
    #[error("Storage read/write error, reason: {error:?}")]
    StorageError { error: StorageError },
    #[error("Mutex/lock error, reason: {reason:?}")]
    LockError { reason: String },
    #[error("State processing error, reason: {reason:?}")]
    ProcessingError { reason: String },
}

impl slog::Value for StateError {
    fn serialize(
        &self,
        _record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

impl<T> From<PoisonError<T>> for StateError {
    fn from(pe: PoisonError<T>) -> Self {
        StateError::LockError {
            reason: format!("{}", pe),
        }
    }
}

impl From<StorageError> for StateError {
    fn from(error: StorageError) -> Self {
        StateError::StorageError { error }
    }
}

impl From<anyhow::Error> for StateError {
    fn from(error: anyhow::Error) -> Self {
        StateError::ProcessingError {
            reason: format!("{}", error),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ApplyBlockBatch {
    pub block_to_apply: Arc<BlockHash>,
    successors: Vec<Arc<BlockHash>>,
}

impl ApplyBlockBatch {
    pub fn one(block_hash: BlockHash) -> Self {
        Self {
            block_to_apply: Arc::new(block_hash),
            successors: Vec::new(),
        }
    }

    pub fn start_batch(block_hash: Arc<BlockHash>, expected_size: usize) -> Self {
        Self {
            block_to_apply: block_hash,
            successors: Vec::with_capacity(expected_size),
        }
    }

    pub fn batch(starting_block: Arc<BlockHash>, successors: Vec<Arc<BlockHash>>) -> Self {
        Self {
            block_to_apply: starting_block,
            successors,
        }
    }

    pub fn add_successor(&mut self, block_hash: Arc<BlockHash>) {
        if !self.successors.contains(&block_hash) {
            self.successors.push(block_hash);
        }
    }

    pub fn batch_total_size(&self) -> usize {
        self.successors_size() + 1
    }

    pub fn successors_size(&self) -> usize {
        self.successors.len()
    }

    pub fn take_all_blocks_to_apply(self) -> Vec<Arc<BlockHash>> {
        let Self {
            block_to_apply,
            mut successors,
        } = self;

        successors.insert(0, block_to_apply);
        successors
    }

    pub fn shift(self) -> Option<ApplyBlockBatch> {
        let Self { mut successors, .. } = self;

        if successors.is_empty() {
            None
        } else {
            let head = successors.remove(0);
            Some(ApplyBlockBatch::batch(head, successors))
        }
    }
}

impl From<ApplyBlockBatch> for (Arc<BlockHash>, Vec<Arc<BlockHash>>) {
    fn from(b: ApplyBlockBatch) -> Self {
        let ApplyBlockBatch {
            block_to_apply,
            successors,
        } = b;
        (block_to_apply, successors)
    }
}

/// HistoryOrderPriority reflects order in history,
/// we want to download oldest blocks at first,
/// we use this atribute to prioritize
/// lowest value means as-soon-as-possible
pub type HistoryOrderPriority = usize;

#[derive(Clone)]
pub struct MissingOperations {
    pub(crate) block_hash: BlockHash,
    pub(crate) validation_passes: HashSet<i8>,
    // TODO: TE-386 - remove not needed
    pub(crate) history_order_priority: HistoryOrderPriority,
    /// ttl retries - in case nobody has header, e.g.: in case of reorg or whatever
    pub(crate) retries: u8,
}

impl MissingOperations {
    /// Returns true, we can do another retry
    pub fn retry(&mut self) -> bool {
        if self.retries == 0 {
            return false;
        }
        self.retries -= 1;
        true
    }
}

impl PartialEq for MissingOperations {
    fn eq(&self, other: &Self) -> bool {
        self.history_order_priority == other.history_order_priority
            && self.block_hash == other.block_hash
    }
}

impl Eq for MissingOperations {}

impl PartialOrd for MissingOperations {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MissingOperations {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.history_order_priority, &self.block_hash)
            .cmp(&(other.history_order_priority, &other.block_hash))
            .reverse()
    }
}

impl From<&MissingOperations> for Vec<OperationsForBlock> {
    fn from(ops: &MissingOperations) -> Self {
        ops.validation_passes
            .iter()
            .map(|vp| OperationsForBlock::new(ops.block_hash.clone(), *vp))
            .collect()
    }
}

#[cfg(test)]
pub mod tests {
    use std::convert::TryInto;
    use std::sync::Arc;

    use super::*;

    #[test]
    fn test_batch() {
        // create batch
        let mut batch = ApplyBlockBatch::batch(block_ref(1), vec![block_ref(2)]);
        batch.add_successor(block_ref(3));
        batch.add_successor(block_ref(4));
        assert_eq!(&block(1), batch.block_to_apply.as_ref());
        assert_eq!(3, batch.successors_size());
        assert_eq!(&block(2), batch.successors[0].as_ref());
        assert_eq!(&block(3), batch.successors[1].as_ref());
        assert_eq!(&block(4), batch.successors[2].as_ref());

        // shift
        let batch = batch.shift().expect("Expected new batch");
        assert_eq!(&block(2), batch.block_to_apply.as_ref());
        assert_eq!(2, batch.successors_size());
        assert_eq!(&block(3), batch.successors[0].as_ref());
        assert_eq!(&block(4), batch.successors[1].as_ref());

        // shift
        let batch = batch.shift().expect("Expected new batch");
        assert_eq!(&block(3), batch.block_to_apply.as_ref());
        assert_eq!(1, batch.successors_size());
        assert_eq!(&block(4), batch.successors[0].as_ref());

        // shift
        let batch = batch.shift().expect("Expected new batch");
        assert_eq!(&block(4), batch.block_to_apply.as_ref());
        assert_eq!(0, batch.successors_size());

        // shift
        assert!(batch.shift().is_none());
    }

    pub(crate) fn block(d: u8) -> BlockHash {
        [d; crypto::hash::HashType::BlockHash.size()]
            .to_vec()
            .try_into()
            .expect("Failed to create BlockHash")
    }

    pub(crate) fn block_ref(d: u8) -> Arc<BlockHash> {
        Arc::new(block(d))
    }

    #[test]
    fn test_retry() {
        let mut data = MissingOperations {
            history_order_priority: 1,
            block_hash: block(5),
            validation_passes: HashSet::new(),
            retries: 5,
        };

        assert!(data.retry());
        assert!(data.retry());
        assert!(data.retry());
        assert!(data.retry());
        assert!(data.retry());
        assert!(!data.retry());
        assert!(!data.retry());
    }

    pub(crate) mod prerequisites {
        use std::convert::{TryFrom, TryInto};
        use std::env;
        use std::fs;
        use std::net::SocketAddr;
        use std::path::{Path, PathBuf};
        use std::sync::atomic::AtomicBool;
        use std::sync::mpsc::{channel, Receiver};
        use std::sync::{Arc, Mutex};
        use std::time::Duration;

        use futures::lock::Mutex as TokioMutex;
        use slog::{Drain, Level, Logger};
        use tezedge_actor_system::actors::*;

        use crypto::hash::{ChainId, CryptoboxPublicKeyHash};
        use networking::p2p::network_channel::NetworkChannelRef;
        use networking::p2p::peer::{BootstrapOutput, Peer};
        use networking::PeerId;
        use shell_integration::OneshotResultCallback;
        use storage::{PersistentStorageRef, StorageInitInfo};
        use tezos_api::environment::*;
        use tezos_api::ffi::{
            GenesisChain, ProtocolOverrides, TezosContextIrminStorageConfiguration,
            TezosContextStorageConfiguration, TezosRuntimeConfiguration,
        };
        use tezos_identity::Identity;
        use tezos_messages::p2p::encoding::prelude::{MetadataMessage, NetworkVersion};
        use tezos_messages::Head;
        use tezos_wrapper::*;

        use crate::chain_feeder;
        use crate::chain_manager::{ChainManager, ChainManagerRef};
        use crate::mempool::{init_mempool_state_storage, MempoolPrevalidatorFactory};
        use crate::shell_channel::ShellChannelRef;
        use crate::state::peer_state::{DataQueuesLimits, PeerState};

        pub(crate) fn test_peer(
            sys: &impl ActorRefFactory,
            network_channel: NetworkChannelRef,
            tokio_runtime: &tokio::runtime::Runtime,
            port: u16,
            log: &Logger,
        ) -> PeerState {
            let socket_address: SocketAddr = format!("127.0.0.1:{}", port)
                .parse()
                .expect("Expected valid ip:port address");

            let node_identity = Arc::new(Identity::generate(0f64).unwrap());
            let peer_public_key_hash: CryptoboxPublicKeyHash =
                node_identity.public_key.public_key_hash().unwrap();
            let peer_id_marker = peer_public_key_hash.to_base58_check();

            let metadata = MetadataMessage::new(false, false);
            let version = NetworkVersion::new("".to_owned(), 0, 0);
            let peer_ref = Peer::actor(
                &peer_id_marker,
                sys,
                network_channel,
                tokio_runtime.handle().clone(),
                BootstrapOutput(
                    Arc::new(TokioMutex::new(None)),
                    Arc::new(TokioMutex::new(None)),
                    peer_public_key_hash.clone(),
                    peer_id_marker.clone(),
                    metadata.clone(),
                    version,
                    socket_address,
                ),
                log,
            )
            .unwrap();

            PeerState::new(
                Arc::new(PeerId::new(
                    peer_ref,
                    peer_public_key_hash,
                    peer_id_marker,
                    socket_address,
                )),
                &metadata,
                DataQueuesLimits {
                    max_queued_block_headers_count: 10,
                    max_queued_block_operations_count: 15,
                },
            )
        }

        pub(crate) fn create_test_actor_system(
            log: Logger,
            handle: tokio::runtime::Handle,
        ) -> ActorSystem {
            SystemBuilder::new()
                .name("create_actor_system")
                .log(log)
                .exec(handle.into())
                .create()
                .expect("Failed to create test actor system")
        }

        pub(crate) fn create_test_tokio_runtime() -> tokio::runtime::Runtime {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create test tokio runtime")
        }

        pub(crate) fn create_logger(level: Level) -> Logger {
            let drain = slog_async::Async::new(
                slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
                    .build()
                    .fuse(),
            )
            .build()
            .filter_level(level)
            .fuse();

            Logger::root(drain, slog::o!())
        }

        pub(crate) fn chain_feeder_mock(
            actor_system: &ActorSystem,
            actor_name: &str,
        ) -> Result<(chain_feeder::ChainFeederRef, Receiver<chain_feeder::Event>), anyhow::Error>
        {
            let (block_applier_event_sender, block_applier_event_receiver) = channel();

            actor_system
                .actor_of_props::<chain_feeder::ChainFeeder>(
                    actor_name,
                    Props::new_args((
                        Arc::new(Mutex::new(block_applier_event_sender)),
                        Arc::new(AtomicBool::new(true)),
                        2,
                    )),
                )
                .map(|feeder| (feeder, block_applier_event_receiver))
                .map_err(|e| {
                    anyhow::format_err!("Failed to create chain_feeder actor mock: {:?}", e)
                })
        }

        pub(crate) fn chain_manager_mock(
            actor_system: Arc<ActorSystem>,
            log: Logger,
            shell_channel: ShellChannelRef,
            network_channel: NetworkChannelRef,
            block_applier: chain_feeder::ChainFeederRef,
            persistent_storage: PersistentStorageRef,
            tokio_runtime: &tokio::runtime::Runtime,
        ) -> Result<ChainManagerRef, anyhow::Error> {
            let init_data = StorageInitInfo {
                chain_id: ChainId::try_from("NetXgtSLGNJvNye")?,
                genesis_block_header_hash: "BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe"
                    .try_into()?,
                patch_context: None,
                replay: None,
                context_stats_db_path: None,
            };

            let tezos_readonly_api_pool =
                Arc::new(create_tezos_readonly_api_pool(tokio_runtime, log.clone())?);

            let current_mempool_state_storage = init_mempool_state_storage();
            let mempool_prevalidator_factory = Arc::new(MempoolPrevalidatorFactory::new(
                actor_system.clone(),
                log,
                persistent_storage.clone(),
                current_mempool_state_storage.clone(),
                tezos_readonly_api_pool.clone(),
                true,
            ));

            let initialize_result: Option<OneshotResultCallback<()>> = None;

            actor_system
                .actor_of_props::<ChainManager>(
                    "chain_manager_mock",
                    Props::new_args((
                        block_applier,
                        network_channel,
                        shell_channel,
                        persistent_storage,
                        tezos_readonly_api_pool,
                        init_data,
                        false,
                        Head::new(
                            "BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe".try_into()?,
                            1,
                            vec![],
                        ),
                        current_mempool_state_storage,
                        0,
                        mempool_prevalidator_factory,
                        Identity::generate(0f64)?.peer_id(),
                        Arc::new(Mutex::new(initialize_result)),
                    )),
                )
                .map_err(|e| e.into())
        }

        fn create_tezos_readonly_api_pool(
            tokio_runtime: &tokio::runtime::Runtime,
            log: Logger,
        ) -> Result<TezosApiConnectionPool, TezosApiConnectionPoolError> {
            TezosApiConnectionPool::new_without_context(
                String::from("create_tezos_readonly_api_pool_for_test"),
                TezosApiConnectionPoolConfiguration {
                    min_connections: 0,
                    max_connections: 1,
                    connection_timeout: Duration::from_secs(3),
                    max_lifetime: Duration::from_secs(60),
                    idle_timeout: Duration::from_secs(60),
                },
                ProtocolEndpointConfiguration::new(
                    TezosRuntimeConfiguration {
                        log_enabled: false,
                        debug_mode: false,
                        compute_context_action_tree_hashes: false,
                    },
                    TezosEnvironmentConfiguration {
                        genesis: GenesisChain {
                            time: "2019-08-06T15:18:56Z".to_string(),
                            block: "BLockGenesisGenesisGenesisGenesisGenesiscde8db4cX94"
                                .to_string(),
                            protocol: "PtBMwNZT94N7gXKw4i273CKcSaBrrBnqnt3RATExNKr9KNX2USV"
                                .to_string(),
                        },
                        bootstrap_lookup_addresses: vec![
                            "bootstrap.zeronet.fun".to_string(),
                            "bootzero.tzbeta.net".to_string(),
                        ],
                        version: "TEZOS_ZERONET_2019-08-06T15:18:56Z".to_string(),
                        protocol_overrides: ProtocolOverrides {
                            user_activated_upgrades: vec![],
                            user_activated_protocol_overrides: vec![],
                        },
                        enable_testchain: true,
                        patch_context_genesis_parameters: None,
                    },
                    false,
                    TezosContextStorageConfiguration::IrminOnly(
                        TezosContextIrminStorageConfiguration {
                            data_dir: prepare_empty_dir("create_tezos_readonly_api_pool_for_test"),
                        },
                    ),
                    "we-dont-need-protocol-runner-here",
                    slog::Level::Debug,
                ),
                tokio_runtime.handle().clone(),
                log.clone(),
            )
        }

        pub fn test_storage_dir_path(dir_name: &str) -> PathBuf {
            let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined");
            Path::new(out_dir.as_str()).join(Path::new(dir_name))
        }

        pub fn prepare_empty_dir(dir_name: &str) -> String {
            let path = test_storage_dir_path(dir_name);
            if path.exists() {
                fs::remove_dir_all(&path)
                    .unwrap_or_else(|_| panic!("Failed to delete directory: {:?}", &path));
            }
            fs::create_dir_all(&path)
                .unwrap_or_else(|_| panic!("Failed to create directory: {:?}", &path));
            String::from(path.to_str().unwrap())
        }
    }
}
