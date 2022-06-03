// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]
#![cfg_attr(feature = "fuzzing", feature(no_coverage))]

use std::path::Path;
use std::sync::Arc;
use std::{
    convert::{TryFrom, TryInto},
    path::PathBuf,
};

pub use rocksdb;
use rocksdb::Cache;
use serde::{Deserialize, Serialize};
use slog::{info, Logger};
use tezos_context_api::{PatchContext, TezosContextStorageConfiguration};
use tezos_messages::p2p::encoding::fitness::Fitness;
use thiserror::Error;

use crypto::{
    base58::FromBase58CheckError,
    hash::{BlockHash, ChainId, ContextHash, FromBytesError, HashType},
};
use tezos_api::environment::{
    get_empty_operation_list_list_hash, TezosEnvironmentConfiguration, TezosEnvironmentError,
};
use tezos_api::ffi::{ApplyBlockRequest, ApplyBlockResponse, CommitGenesisResult};
use tezos_messages::p2p::binary_message::{BinaryRead, BinaryWrite, MessageHash, MessageHashError};
use tezos_messages::p2p::encoding::prelude::BlockHeader;
use tezos_messages::Head;

pub use crate::block_meta_storage::{
    BlockAdditionalData, BlockMetaStorage, BlockMetaStorageKV, BlockMetaStorageReader,
};
pub use crate::block_storage::{BlockJsonData, BlockStorage, BlockStorageReader};
pub use crate::chain_meta_storage::{ChainMetaStorage, ChainMetaStorageReader};
use crate::commit_log::{CommitLogError, CommitLogs};
pub use crate::constants_storage::ConstantsStorage;
pub use crate::cycle_eras_storage::CycleErasStorage;
pub use crate::cycle_storage::CycleMetaStorage;
use crate::database::tezedge_database::TezedgeDatabase;
pub use crate::mempool_storage::{MempoolStorage, MempoolStorageKV};
pub use crate::operations_meta_storage::{OperationsMetaStorage, OperationsMetaStorageKV};
pub use crate::operations_storage::{
    OperationKey, OperationsStorage, OperationsStorageKV, OperationsStorageReader,
};
pub use crate::persistent::database::{Direction, IteratorMode};
use crate::persistent::sequence::{SequenceError, Sequences};
use crate::persistent::{DBError, Decoder, Encoder, SchemaError};
pub use crate::predecessor_storage::PredecessorStorage;
pub use crate::shell_automaton::*;
pub use crate::system_storage::SystemStorage;

pub mod block_meta_storage;
pub mod block_storage;
pub mod chain_meta_storage;
pub mod commit_log;
pub mod constants_storage;
pub mod cycle_eras_storage;
pub mod cycle_storage;
pub mod database;
pub mod mempool_storage;
pub mod operations_meta_storage;
pub mod operations_storage;
pub mod persistent;
pub mod predecessor_storage;
pub mod reward_storage;
mod shell_automaton;
pub mod system_storage;

/// Extension of block header with block hash
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct BlockHeaderWithHash {
    pub hash: BlockHash,
    pub header: Arc<BlockHeader>,
}

impl BlockHeaderWithHash {
    /// Create block header extensions from plain block header
    /// TODO https://viablesystems.atlassian.net/browse/TE-674
    pub fn new(block_header: BlockHeader) -> Result<Self, MessageHashError> {
        let hash = if block_header.level() == 0 {
            // For genesis block, we don't get the genesis block hash by
            // hashing the header. Instead we need to use predecessor.
            block_header.predecessor().clone()
        } else if let Some(hash) = block_header.hash().as_ref() {
            hash.as_slice().try_into()?
        } else {
            block_header.message_hash()?.try_into()?
        };
        let header = Arc::new(block_header);
        Ok(BlockHeaderWithHash { hash, header })
    }
}

impl TryFrom<BlockHeader> for BlockHeaderWithHash {
    type Error = MessageHashError;

    fn try_from(value: BlockHeader) -> Result<Self, Self::Error> {
        BlockHeaderWithHash::new(value)
    }
}

impl Encoder for BlockHeaderWithHash {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut result = vec![];
        result.extend(self.hash.as_ref());
        result.extend(
            self.header
                .as_bytes()
                .map_err(|_| SchemaError::EncodeError)?,
        );
        Ok(result)
    }
}

impl Decoder for BlockHeaderWithHash {
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() < HashType::BlockHash.size() {
            return Err(SchemaError::DecodeError);
        }
        let hash = bytes[0..HashType::BlockHash.size()].to_vec();
        let header = BlockHeader::from_bytes(&bytes[HashType::BlockHash.size()..])
            .map_err(|_| SchemaError::DecodeError)?;
        Ok(BlockHeaderWithHash {
            hash: hash.try_into()?,
            header: Arc::new(header),
        })
    }
}

/// Possible errors for storage
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Database error: {error}")]
    DBError { error: DBError },
    #[error("Commit log error: {error:?}")]
    CommitLogError { error: CommitLogError },
    #[error("Key is missing in storage, when: {when}")]
    MissingKey { when: String },
    #[error("Column is not valid")]
    InvalidColumn,
    #[error("Sequence generator failed: {error}")]
    SequenceError { error: SequenceError },
    #[error("Tezos environment configuration error: {error}")]
    TezosEnvironmentError { error: TezosEnvironmentError },
    #[error("Message hash error: {error}")]
    MessageHashError { error: MessageHashError },
    #[error("Error constructing hash: {error}")]
    HashError { error: FromBytesError },
    #[error("Error decoding hash: {error}")]
    HashDecodeError { error: FromBase58CheckError },
    #[error("Database error: {error:?}")]
    MainDBError { error: database::error::Error },
    #[error("Deserialization: {error}")]
    SerdeJsonError { error: serde_json::Error },
}

impl From<DBError> for StorageError {
    fn from(error: DBError) -> Self {
        StorageError::DBError { error }
    }
}

impl From<MessageHashError> for StorageError {
    fn from(error: MessageHashError) -> Self {
        StorageError::MessageHashError { error }
    }
}

impl From<CommitLogError> for StorageError {
    fn from(error: CommitLogError) -> Self {
        StorageError::CommitLogError { error }
    }
}

impl From<SchemaError> for StorageError {
    fn from(error: SchemaError) -> Self {
        StorageError::DBError {
            error: error.into(),
        }
    }
}

impl From<SequenceError> for StorageError {
    fn from(error: SequenceError) -> Self {
        StorageError::SequenceError { error }
    }
}

impl From<TezosEnvironmentError> for StorageError {
    fn from(error: TezosEnvironmentError) -> Self {
        StorageError::TezosEnvironmentError { error }
    }
}

impl From<FromBytesError> for StorageError {
    fn from(error: FromBytesError) -> Self {
        StorageError::HashError { error }
    }
}

impl From<FromBase58CheckError> for StorageError {
    fn from(error: FromBase58CheckError) -> Self {
        StorageError::HashDecodeError { error }
    }
}
impl From<database::error::Error> for StorageError {
    fn from(error: database::error::Error) -> Self {
        StorageError::MainDBError { error }
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(error: serde_json::Error) -> Self {
        StorageError::SerdeJsonError { error }
    }
}

impl slog::Value for StorageError {
    fn serialize(
        &self,
        _record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Replay {
    pub from_block: Option<BlockHash>,
    pub to_block: BlockHash,
    pub fail_above: std::time::Duration,
}

#[derive(Debug, Clone)]
pub struct StorageSnapshot {
    pub block: Option<BlockReference>,
    pub target_path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum BlockReference {
    BlockHash(BlockHash),
    Level(u32),
    OffsetFromHead(u32),
}

/// Struct represent init information about storage on startup
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageInitInfo {
    pub chain_id: ChainId,
    pub genesis_block_header_hash: BlockHash,
    pub patch_context: Option<PatchContext>,
    pub context_stats_db_path: Option<PathBuf>,
    pub replay: Option<Replay>,
}

/// Resolve main chain id and genesis header from configuration
pub fn resolve_storage_init_chain_data(
    tezos_env: &TezosEnvironmentConfiguration,
    storage_db_path: &Path,
    context_storage_configuration: &TezosContextStorageConfiguration,
    patch_context: &Option<PatchContext>,
    context_stats_db_path: &Option<PathBuf>,
    replay: &Option<Replay>,
    log: &Logger,
) -> Result<StorageInitInfo, StorageError> {
    let init_data = StorageInitInfo {
        chain_id: tezos_env.main_chain_id()?,
        genesis_block_header_hash: tezos_env.genesis_header_hash()?,
        patch_context: patch_context.clone(),
        replay: replay.clone(),
        context_stats_db_path: context_stats_db_path.clone(),
    };

    info!(
        log,
        "Storage based on data";
        "chain_name" => &tezos_env.version,
        "init_data.chain_id" => format!("{:?}", init_data.chain_id.to_base58_check()),
        "init_data.genesis_header" => format!("{:?}", init_data.genesis_block_header_hash.to_base58_check()),
        "storage_db_path" => format!("{:?}", storage_db_path),
        "context_storage_configuration" => format!("{:?}", context_storage_configuration),
        "context_stats_db_path" => format!("{:?}", context_stats_db_path),
        "patch_context" => match patch_context {
                Some(pc) => format!("{:?}", pc),
                None => "-none-".to_string()
        },
    );
    Ok(init_data)
}

/// Collects complete data for applying block, if not complete, return None
#[inline]
pub fn prepare_block_apply_request(
    block_hash: &BlockHash,
    chain_id: ChainId,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    operations_storage: &OperationsStorage,
    predecessor_data_cache: Option<(Arc<BlockHeaderWithHash>, BlockAdditionalData)>,
) -> Result<
    (
        ApplyBlockRequest,
        block_meta_storage::Meta,
        Arc<BlockHeaderWithHash>,
    ),
    StorageError,
> {
    // get block header
    let block = match block_storage.get(block_hash)? {
        Some(block) => Arc::new(block),
        None => {
            return Err(StorageError::MissingKey {
                when: format!(
                    "prepare_apply_request (block header not found, block_hash: {}",
                    block_hash.to_base58_check()
                ),
            });
        }
    };

    // get block_metadata
    let block_meta = match block_meta_storage.get(block_hash)? {
        Some(meta) => meta,
        None => {
            return Err(StorageError::MissingKey {
                when: format!(
                    "prepare_apply_request (block header metadata not, block_hash: {}",
                    block_hash.to_base58_check()
                ),
            });
        }
    };

    // get operations
    let operations = operations_storage.get_operations(block_hash)?;

    // resolve predecessor data
    let (
        predecessor,
        (
            predecessor_block_metadata_hash,
            predecessor_ops_metadata_hash,
            predecessor_max_operations_ttl,
        ),
    ) = resolve_block_data(
        block.header.predecessor(),
        block_storage,
        block_meta_storage,
        predecessor_data_cache,
    )
    .map(|(block, additional_data)| (block, additional_data.into()))?;

    Ok((
        ApplyBlockRequest {
            chain_id,
            block_header: block.header.as_ref().clone(),
            pred_header: predecessor.header.as_ref().clone(),
            operations: ApplyBlockRequest::convert_operations(operations),
            max_operations_ttl: predecessor_max_operations_ttl as i32,
            predecessor_block_metadata_hash,
            predecessor_ops_metadata_hash,
        },
        block_meta,
        block,
    ))
}

#[inline]
fn resolve_block_data(
    block_hash: &BlockHash,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    block_data_cache: Option<(Arc<BlockHeaderWithHash>, BlockAdditionalData)>,
) -> Result<(Arc<BlockHeaderWithHash>, BlockAdditionalData), StorageError> {
    // check cache at first
    if let Some(cached) = block_data_cache {
        // if cached data are the same as requested, then use it from cache
        if block_hash.eq(&cached.0.hash) {
            return Ok(cached);
        }
    }
    // load data from database
    let block = match block_storage.get(block_hash)? {
        Some(header) => Arc::new(header),
        None => {
            return Err(StorageError::MissingKey {
                when: format!(
                    "resolve_block_data (block header not found, block_hash: {}",
                    block_hash.to_base58_check()
                ),
            });
        }
    };

    // predecessor additional data
    let additional_data = match block_meta_storage.get_additional_data(block_hash)? {
        Some(additional_data) => additional_data,
        None => {
            return Err(StorageError::MissingKey {
                    when: format!("resolve_block_data (block header metadata not found (block was not applied), block_hash: {}", block_hash.to_base58_check()),
                });
        }
    };

    Ok((block, additional_data))
}

/// Stores apply result to storage and mark block as applied, if everythnig is ok.
pub fn store_applied_block_result(
    chain_meta_storage: &ChainMetaStorage,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    block_hash: &BlockHash,
    block_fitness: Fitness,
    block_result: ApplyBlockResponse,
    block_metadata: &mut block_meta_storage::Meta,
    cycle_meta_storage: &CycleMetaStorage,
    cycle_eras_storage: &CycleErasStorage,
    constants_storage: &ConstantsStorage,
) -> Result<BlockAdditionalData, StorageError> {
    // store result data - json and additional data
    let block_json_data = BlockJsonData::new(
        block_result.block_header_proto_json,
        block_result.block_header_proto_metadata_bytes,
        block_result.operations_proto_metadata_bytes,
    );
    block_storage.put_block_json_data(block_hash, block_json_data)?;

    // store additional data
    let block_additional_data = BlockAdditionalData::new(
        block_result.max_operations_ttl.try_into().unwrap(),
        block_result.last_allowed_fork_level,
        block_result.protocol_hash,
        block_result.next_protocol_hash.clone(),
        block_result.block_metadata_hash,
        {
            // Note: Ocaml introduces this two attributes (block_metadata_hash, ops_metadata_hash) in 008 edo
            //       So, we need to add the same handling, because this attributes contributes to context_hash
            //       They, store it, only if [`validation_passes > 0`], this measn that we have some operations
            match &block_result.ops_metadata_hashes {
                Some(hashes) => {
                    if hashes.is_empty() {
                        None
                    } else {
                        block_result.ops_metadata_hash
                    }
                }
                None => None,
            }
        },
        block_result.ops_metadata_hashes,
    );
    block_meta_storage.put_block_additional_data(block_hash, &block_additional_data)?;

    // TODO: check context checksum or context_hash

    // populate predecessor storage
    block_meta_storage.store_predecessors(block_hash, block_metadata)?;

    // populate cycle data if is present in the response
    for cycle_data in block_result.cycle_rolls_owner_snapshots.into_iter() {
        cycle_meta_storage.store_cycle_data(cycle_data)?;
    }

    // store new constants if they are present
    if let Some(constants) = block_result.new_protocol_constants_json {
        constants_storage
            .store_constants_data(block_result.next_protocol_hash.clone(), constants)?;
    }

    // store new cycle eras if they are present
    if let Some(new_cycle_eras) = block_result.new_cycle_eras_json {
        cycle_eras_storage
            .store_cycle_eras_data(block_result.next_protocol_hash.clone(), new_cycle_eras)?;
    }

    // if everything is stored and ok, we can considere this block as applied
    // mark current head as applied
    block_metadata.set_is_applied(true);
    block_meta_storage.put(block_hash, block_metadata)?;

    // TODO(zura): maybe move to separate storage call.
    chain_meta_storage.set_current_head(
        block_metadata.chain_id(),
        Head::new(block_hash.clone(), block_metadata.level(), block_fitness),
    )?;

    // return additional data for later use
    Ok(block_additional_data)
}

/// Stores commit_genesis result to storage and mark genesis block as applied, if everythnig is ok.
/// !Important, this rewrites context_hash on stored genesis - because in initialize_storage_with_genesis_block we stored wiht Context_hash_zero
/// And context hash of block is used for appling of successor
pub fn store_commit_genesis_result(
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    chain_meta_storage: &ChainMetaStorage,
    operations_meta_storage: &OperationsMetaStorage,
    init_storage_data: &StorageInitInfo,
    block_result: CommitGenesisResult,
) -> Result<(), StorageError> {
    // store data for genesis
    let genesis_block_hash = &init_storage_data.genesis_block_header_hash;
    let chain_id = &init_storage_data.chain_id;

    // if everything is stored and ok, we can considere genesis block as applied
    // if storage is empty, initialize with genesis
    block_meta_storage.put(
        genesis_block_hash,
        &block_meta_storage::Meta::genesis_meta(genesis_block_hash, chain_id, true),
    )?;
    operations_meta_storage.put(
        genesis_block_hash,
        &operations_meta_storage::Meta::genesis_meta(),
    )?;

    // store result data - json and additional data
    let block_json_data = BlockJsonData::new(
        block_result.block_header_proto_json,
        block_result.block_header_proto_metadata_bytes,
        block_result.operations_proto_metadata_bytes,
    );
    block_storage.put_block_json_data(genesis_block_hash, block_json_data)?;

    // set genesis as current head - it is empty storage
    match block_storage.get(genesis_block_hash)? {
        Some(genesis) => {
            let head = Head::new(
                genesis.hash,
                genesis.header.level(),
                genesis.header.fitness().clone(),
            );

            // init chain data
            chain_meta_storage.set_genesis(chain_id, head.clone())?;
            chain_meta_storage.set_caboose(chain_id, head.clone())?;
            chain_meta_storage.set_current_head(chain_id, head)?;

            Ok(())
        }
        None => Err(StorageError::MissingKey {
            when: "store_commit_genesis_result".into(),
        }),
    }
}

/// Genesis block needs extra handling because predecessor of the genesis block is genesis itself.
/// Which means that successor of the genesis block is also genesis block. By combining those
/// two statements we get cyclic relationship and everything breaks..
///
/// Genesis header is also "applied", but with special case "commit_genesis", which initialize protocol context.
/// We must ensure this, because applying next blocks depends on Context (identified by ContextHash) of predecessor.
///
/// So when "commit_genesis" event occurrs, we must use Context_hash and create genesis header within.
/// Commit_genesis also updates block json/additional metadata resolved by protocol.
pub fn initialize_storage_with_genesis_block(
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    init_storage_data: &StorageInitInfo,
    tezos_env: &TezosEnvironmentConfiguration,
    context_hash: &ContextHash,
    log: &Logger,
) -> Result<BlockHeaderWithHash, StorageError> {
    // TODO: check context checksum or context_hash

    // store genesis
    let genesis_with_hash = BlockHeaderWithHash {
        hash: init_storage_data.genesis_block_header_hash.clone(),
        header: Arc::new(
            tezos_env
                .genesis_header(context_hash.clone(), get_empty_operation_list_list_hash()?)?,
        ),
    };
    let _ = block_storage.put_block_header(&genesis_with_hash)?;

    // store additional data
    let genesis_additional_data = tezos_env.genesis_additional_data()?;
    let block_additional_data = BlockAdditionalData::new(
        genesis_additional_data.max_operations_ttl,
        genesis_additional_data.last_allowed_fork_level,
        genesis_additional_data.protocol_hash,
        genesis_additional_data.next_protocol_hash,
        None,
        None,
        None,
    );
    block_meta_storage
        .put_block_additional_data(&genesis_with_hash.hash, &block_additional_data)?;

    info!(log,
        "Storage initialized with genesis block";
        "genesis" => genesis_with_hash.hash.to_base58_check(),
        "context_hash" => context_hash.to_base58_check(),
    );
    Ok(genesis_with_hash)
}

pub fn hydrate_current_head(
    chain_id: &ChainId,
    persistent_storage: &PersistentStorage,
) -> Result<BlockHeaderWithHash, StorageError> {
    // check last stored current_head
    let current_head = match ChainMetaStorage::new(persistent_storage).get_current_head(chain_id)? {
        Some(head) => head,
        None => {
            return Err(StorageError::MissingKey {
                when: "current_head".into(),
            })
        }
    };

    // get block_header data
    match BlockStorage::new(persistent_storage).get(current_head.block_hash())? {
        Some(block) => Ok(block),
        None => Err(StorageError::MissingKey {
            when: "current_head_header".into(),
        }),
    }
}

/// Helper module to easily initialize databases
pub mod initializer {
    use std::path::PathBuf;
    use std::sync::Arc;

    use rocksdb::{Cache, ColumnFamilyDescriptor, DB};
    use slog::{error, Logger};

    use crate::database::error::Error as DatabaseError;
    use crate::database::tezedge_database::{TezedgeDatabase, TezedgeDatabaseBackendConfiguration};
    use crate::persistent::database::{open_kv, RocksDbKeyValueSchema};
    use crate::persistent::{open_main_db, DBError, DbConfiguration};
    use crate::{StorageError, SystemStorage};
    use crypto::hash::ChainId;

    // IMPORTANT: Cache object must live at least as long as DB (returned by open_kv)
    pub type GlobalRocksDbCacheHolder = Vec<RocksDbCache>;
    pub type RocksDbCache = Cache;

    /// Factory for creation of grouped column family descriptors
    pub trait RocksDbColumnFactory {
        fn create(&self, cache: &RocksDbCache) -> Vec<ColumnFamilyDescriptor>;
    }

    /// Tables initializer for all operational datbases
    #[derive(Debug, Clone)]
    pub struct DbsRocksDbTableInitializer;

    /// Tables initializer for context action rocksdb k-v store (if configured)
    #[derive(Debug, Clone)]
    pub struct ContextActionsRocksDbTableInitializer;

    impl RocksDbColumnFactory for DbsRocksDbTableInitializer {
        fn create(&self, cache: &RocksDbCache) -> Vec<ColumnFamilyDescriptor> {
            vec![
                crate::block_storage::BlockPrimaryIndex::descriptor(cache),
                crate::block_storage::BlockByLevelIndex::descriptor(cache),
                crate::block_storage::BlockByContextHashIndex::descriptor(cache),
                crate::BlockMetaStorage::descriptor(cache),
                crate::OperationsStorage::descriptor(cache),
                crate::OperationsMetaStorage::descriptor(cache),
                crate::SystemStorage::descriptor(cache),
                crate::persistent::sequence::Sequences::descriptor(cache),
                crate::MempoolStorage::descriptor(cache),
                crate::ChainMetaStorage::descriptor(cache),
                crate::PredecessorStorage::descriptor(cache),
                crate::BlockAdditionalData::descriptor(cache),
                crate::CycleMetaStorage::descriptor(cache),
                crate::CycleErasStorage::descriptor(cache),
                crate::ConstantsStorage::descriptor(cache),
                crate::ShellAutomatonStateStorage::descriptor(cache),
                crate::ShellAutomatonActionStorage::descriptor(cache),
                crate::ShellAutomatonActionMetaStorage::descriptor(cache),
                crate::reward_storage::RewardStorage::descriptor(cache),
            ]
        }
    }

    #[derive(Debug, Clone)]
    pub struct RocksDbConfig<C: RocksDbColumnFactory> {
        pub cache_size: usize,
        pub expected_db_version: i64,
        pub db_path: PathBuf,
        pub columns: C,
        pub threads: Option<usize>,
    }

    pub fn initialize_rocksdb<Factory: RocksDbColumnFactory>(
        // TODO - TE-498 - remove _log
        _log: &Logger,
        // TODO - TE-498 - remove cache
        cache: &Cache,
        config: &RocksDbConfig<Factory>,
        // TODO - TE-498 - remove _expected_main_chain
        _expected_main_chain: &MainChain,
    ) -> Result<Arc<DB>, DBError> {
        let db = open_kv(
            &config.db_path,
            config.columns.create(cache),
            &DbConfiguration {
                max_threads: config.threads,
            },
        )
        .map(Arc::new)?;

        Ok(db)
    }

    pub struct MainChain {
        chain_id: ChainId,
        chain_name: String,
    }

    impl MainChain {
        pub fn new(chain_id: ChainId, chain_name: String) -> Self {
            Self {
                chain_id,
                chain_name,
            }
        }
    }

    pub fn initialize_maindb<C: RocksDbColumnFactory>(
        log: &Logger,
        kv: Option<Arc<DB>>,
        config: &RocksDbConfig<C>,
        db_version: i64,
        expected_main_chain: &MainChain,
        backend_config: TezedgeDatabaseBackendConfiguration,
    ) -> Result<Arc<TezedgeDatabase>, DatabaseError> {
        let db = Arc::new(open_main_db(kv, config, backend_config, log.clone())?);

        match check_database_compatibility(db.clone(), db_version, expected_main_chain, log) {
            Ok(false) => Err(DatabaseError::DatabaseIncompatibility {
                name: format!("Database is incompatible with version {}", db_version),
            }),
            Err(e) => Err(DatabaseError::DatabaseIncompatibility {
                name: format!("Failed to verify database compatibility reason: '{}'", e),
            }),
            _ => Ok(db),
        }
    }

    fn check_database_compatibility(
        db: Arc<TezedgeDatabase>,
        expected_database_version: i64,
        expected_main_chain: &MainChain,
        log: &Logger,
    ) -> Result<bool, StorageError> {
        let mut system_info = SystemStorage::new(db);
        let (db_version_ok, found_database_version) = match system_info.get_db_version()? {
            Some(db_version) => {
                // TODO: TE-608 - refactor this 19/20 fix
                match expected_database_version {
                    19 => (db_version == 19 || db_version == 20, db_version),
                    20 => (db_version == 19 || db_version == 20, db_version),
                    _ => (db_version == expected_database_version, db_version),
                }
            }
            None => {
                system_info.set_db_version(expected_database_version)?;
                (true, expected_database_version)
            }
        };
        if !db_version_ok {
            error!(log, "Incompatible database version found (expected {}, found {}). Please re-sync your node to empty storage - see configuration!", expected_database_version, found_database_version);
        }

        let tezos_env_main_chain_id = &expected_main_chain.chain_id;
        let tezos_env_main_chain_name = &expected_main_chain.chain_name;

        let (chain_id_ok, previous_chain_name, requested_chain_name) =
            match system_info.get_chain_id()? {
                Some(chain_id) => {
                    let previous_chain_name = match system_info.get_chain_name()? {
                        Some(chn) => chn,
                        None => "-unknown-".to_string(),
                    };

                    if chain_id == *tezos_env_main_chain_id
                        && previous_chain_name.eq(tezos_env_main_chain_name.as_str())
                    {
                        (true, previous_chain_name, tezos_env_main_chain_name)
                    } else {
                        (false, previous_chain_name, tezos_env_main_chain_name)
                    }
                }
                None => {
                    system_info.set_chain_id(tezos_env_main_chain_id)?;
                    system_info.set_chain_name(tezos_env_main_chain_name)?;
                    (true, "-none-".to_string(), tezos_env_main_chain_name)
                }
            };

        if !chain_id_ok {
            error!(log, "Current database was previously created for another chain. Please re-sync your node to empty storage - see configuration!";
                        "requested_chain" => requested_chain_name,
                        "previous_chain" => previous_chain_name
            );
        }

        Ok(db_version_ok && chain_id_ok)
    }
}

#[derive(Clone)]
pub struct PersistentStorage {
    /// key-value store for main db
    main_db: Arc<TezedgeDatabase>,
    /// commit log store for storing plain block header data
    clog: Arc<CommitLogs>,
    /// autoincrement  id generators
    seq: Arc<Sequences>,
}

impl PersistentStorage {
    pub fn new(main_db: Arc<TezedgeDatabase>, clog: Arc<CommitLogs>, seq: Arc<Sequences>) -> Self {
        Self { clog, main_db, seq }
    }

    #[inline]
    pub fn main_db(&self) -> Arc<TezedgeDatabase> {
        self.main_db.clone()
    }

    #[inline]
    pub fn clog(&self) -> Arc<CommitLogs> {
        self.clog.clone()
    }

    #[inline]
    pub fn seq(&self) -> Arc<Sequences> {
        self.seq.clone()
    }

    pub fn flush_dbs(&mut self) {
        if Arc::strong_count(&self.clog) == 1 {
            self.clog.flush_checked();
        }
        if Arc::strong_count(&self.main_db) == 1 {
            self.main_db.flush_checked();
        }
    }
}

impl Drop for PersistentStorage {
    fn drop(&mut self) {
        self.flush_dbs();
    }
}

pub mod tests_common {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::{env, fs};

    use anyhow::Error;

    use slog::{Drain, Level, Logger};

    use crate::block_storage;
    use crate::chain_meta_storage::ChainMetaStorage;
    use crate::mempool_storage::MempoolStorage;
    use crate::persistent::database::{open_kv, RocksDbKeyValueSchema};
    use crate::persistent::sequence::Sequences;
    use crate::persistent::{open_cl, CommitLogSchema, DbConfiguration};
    use crate::reward_storage::RewardStorage;

    use super::*;
    use crate::database::tezedge_database::TezedgeDatabaseBackendOptions;

    pub struct TmpStorage {
        persistent_storage: PersistentStorage,
        path: TmpStoragePath,
    }

    struct TmpStoragePath {
        path: PathBuf,
    }

    impl TmpStorage {
        pub fn create_to_out_dir(dir_name: &str) -> Result<Self, Error> {
            let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined - check build.rs");
            let path = Path::new(out_dir.as_str()).join(Path::new(dir_name));
            Self::create(path)
        }

        pub fn create<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
            Self::initialize(path, true)
        }

        pub fn initialize<P: AsRef<Path>>(path: P, remove_if_exists: bool) -> Result<Self, Error> {
            // logger
            let log_level = log_level();
            let log = create_logger(log_level);

            let path = path.as_ref().to_path_buf();
            // remove previous data if exists
            if Path::new(&path).exists() && remove_if_exists {
                fs::remove_dir_all(&path).unwrap();
            }

            let cfg = DbConfiguration::default();

            // create common RocksDB block cache to be shared among column families
            let db_cache = Cache::new_lru_cache(128 * 1024 * 1024)?; // 128 MB
            let backend = if cfg!(feature = "maindb-backend-rocksdb") {
                let kv = Arc::new(open_kv(
                    path.join("db"),
                    vec![
                        block_storage::BlockPrimaryIndex::descriptor(&db_cache),
                        block_storage::BlockByLevelIndex::descriptor(&db_cache),
                        block_storage::BlockByContextHashIndex::descriptor(&db_cache),
                        BlockMetaStorage::descriptor(&db_cache),
                        OperationsStorage::descriptor(&db_cache),
                        OperationsMetaStorage::descriptor(&db_cache),
                        SystemStorage::descriptor(&db_cache),
                        Sequences::descriptor(&db_cache),
                        MempoolStorage::descriptor(&db_cache),
                        ChainMetaStorage::descriptor(&db_cache),
                        PredecessorStorage::descriptor(&db_cache),
                        BlockAdditionalData::descriptor(&db_cache),
                        CycleErasStorage::descriptor(&db_cache),
                        CycleMetaStorage::descriptor(&db_cache),
                        ConstantsStorage::descriptor(&db_cache),
                        ShellAutomatonStateStorage::descriptor(&db_cache),
                        ShellAutomatonActionStorage::descriptor(&db_cache),
                        ShellAutomatonActionMetaStorage::descriptor(&db_cache),
                        RewardStorage::descriptor(&db_cache),
                    ],
                    &cfg,
                )?);
                TezedgeDatabaseBackendOptions::RocksDB(
                    database::rockdb_backend::RocksDBBackend::from_db(kv)?,
                )
            } else if cfg!(feature = "maindb-backend-sled") {
                TezedgeDatabaseBackendOptions::SledDB(database::sled_backend::SledDBBackend::new(
                    path.join("db"),
                )?)
            } else if cfg!(feature = "maindb-backend-edgekv") {
                TezedgeDatabaseBackendOptions::EdgeKV(database::edgekv_backend::EdgeKVBackend::new(
                    path.join("db"),
                    vec![
                        block_storage::BlockPrimaryIndex::name(),
                        block_storage::BlockByLevelIndex::name(),
                        block_storage::BlockByContextHashIndex::name(),
                        BlockMetaStorage::name(),
                        OperationsStorage::name(),
                        OperationsMetaStorage::name(),
                        SystemStorage::name(),
                        Sequences::name(),
                        MempoolStorage::name(),
                        ChainMetaStorage::name(),
                        PredecessorStorage::name(),
                        BlockAdditionalData::name(),
                        CycleErasStorage::name(),
                        CycleMetaStorage::name(),
                        ConstantsStorage::name(),
                        ShellAutomatonStateStorage::name(),
                        ShellAutomatonActionStorage::name(),
                        ShellAutomatonActionMetaStorage::name(),
                        RewardStorage::name(),
                    ],
                )?)
            } else {
                let kv = Arc::new(open_kv(
                    path.join("db"),
                    vec![
                        block_storage::BlockPrimaryIndex::descriptor(&db_cache),
                        block_storage::BlockByLevelIndex::descriptor(&db_cache),
                        block_storage::BlockByContextHashIndex::descriptor(&db_cache),
                        BlockMetaStorage::descriptor(&db_cache),
                        OperationsStorage::descriptor(&db_cache),
                        OperationsMetaStorage::descriptor(&db_cache),
                        SystemStorage::descriptor(&db_cache),
                        Sequences::descriptor(&db_cache),
                        MempoolStorage::descriptor(&db_cache),
                        ChainMetaStorage::descriptor(&db_cache),
                        PredecessorStorage::descriptor(&db_cache),
                        BlockAdditionalData::descriptor(&db_cache),
                        CycleErasStorage::descriptor(&db_cache),
                        CycleMetaStorage::descriptor(&db_cache),
                        ConstantsStorage::descriptor(&db_cache),
                        ShellAutomatonStateStorage::descriptor(&db_cache),
                        ShellAutomatonActionStorage::descriptor(&db_cache),
                        ShellAutomatonActionMetaStorage::descriptor(&db_cache),
                        RewardStorage::descriptor(&db_cache),
                    ],
                    &cfg,
                )?);
                TezedgeDatabaseBackendOptions::RocksDB(
                    database::rockdb_backend::RocksDBBackend::from_db(kv)?,
                )
            };
            // db storage - is used for db and sequences

            let maindb = Arc::new(TezedgeDatabase::new(backend, log.clone()));
            // commit log storage
            let clog = open_cl(&path, vec![BlockStorage::descriptor()], log)?;

            Ok(Self {
                persistent_storage: PersistentStorage::new(
                    maindb.clone(),
                    Arc::new(clog),
                    Arc::new(Sequences::new(maindb, 1000)),
                ),
                path: TmpStoragePath { path },
            })
        }

        pub fn storage(&self) -> &PersistentStorage {
            &self.persistent_storage
        }

        pub fn path(&self) -> &PathBuf {
            &self.path.path
        }
    }

    pub fn create_logger(level: Level) -> Logger {
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

    pub fn log_level() -> Level {
        env::var("LOG_LEVEL")
            .unwrap_or_else(|_| "info".to_string())
            .parse::<Level>()
            .unwrap()
    }

    #[cfg(test)]
    mod tests {
        use tezos_messages::p2p::encoding::fitness::Fitness;

        use super::TmpStorage;
        use crate::{ChainId, ChainMetaStorage, Head};
        use std::{
            convert::{TryFrom, TryInto},
            sync::{
                atomic::{AtomicBool, Ordering},
                Arc,
            },
            thread,
            time::Duration,
        };

        #[test]
        fn test_storage_stuck() {
            let tmp_storage = TmpStorage::create("target/tmp_storage").expect("create a storage");
            let index = ChainMetaStorage::new(tmp_storage.storage());
            let running = Arc::new(AtomicBool::new(true));
            let num_threads = 4;
            let threads = (0..num_threads)
                .map(|i| {
                    let running = running.clone();
                    let index = index.clone();
                    thread::spawn(move || {
                        let chain_id =
                            |x: u32| ChainId::try_from(x.to_be_bytes().to_vec()).unwrap();
                        let block = |level| {
                            Head::new(
                                "BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe"
                                    .try_into()
                                    .unwrap(),
                                level * 2,
                                Fitness::default(),
                            )
                        };

                        let mut level = i as i32;
                        let mut id = 0x123456 + i;
                        while running.load(Ordering::SeqCst) {
                            index.set_current_head(&chain_id(id), block(level)).unwrap();
                            id += num_threads;
                            level += num_threads as i32;
                        }
                    })
                })
                .collect::<Vec<_>>();

            thread::sleep(Duration::from_secs(1));
            running.store(false, Ordering::SeqCst);

            for thread in threads {
                thread.join().unwrap();
            }

            drop(tmp_storage);
        }
    }
}
