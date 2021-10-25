// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::borrow::Cow;
use std::sync::Arc;

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, ContextHash};

use crate::commit_log::{CommitLogWithSchema, Location};
use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezedgeDatabaseWithIterator};
use crate::persistent::database::RocksDbKeyValueSchema;
use crate::persistent::{BincodeEncoded, CommitLogSchema, KeyValueSchema};
use crate::predecessor_storage::PredecessorSearch;
use crate::{
    BlockHeaderWithHash, IteratorMode, PersistentStorage, PredecessorStorage, StorageError,
};

/// Store block header data in a key-value store and into commit log.
/// The value is first inserted into commit log, which returns a location of the newly inserted value.
/// That location is then stored as a value in the key-value store.
///
/// The assumption is that, if primary_index contains block_hash, then also commit_log contains header data
#[derive(Clone)]
pub struct BlockStorage {
    primary_index: BlockPrimaryIndex,
    predecessor_storage: PredecessorStorage,
    by_context_hash_index: BlockByContextHashIndex,
    clog: Arc<BlockStorageCommitLog>,
}

pub type BlockStorageCommitLog = dyn CommitLogWithSchema<BlockStorage> + Sync + Send;
pub type BlockLevel = i32;

#[derive(Clone, Getters, Serialize, Deserialize, Debug)]
pub struct BlockJsonData {
    #[get = "pub"]
    pub block_header_proto_json: String,
    #[get = "pub"]
    pub block_header_proto_metadata_bytes: Vec<u8>,
    #[get = "pub"]
    pub operations_proto_metadata_bytes: Vec<Vec<Vec<u8>>>,
}

impl BlockJsonData {
    pub fn new(
        block_header_proto_json: String,
        block_header_proto_metadata_bytes: Vec<u8>,
        operations_proto_metadata_bytes: Vec<Vec<Vec<u8>>>,
    ) -> Self {
        Self {
            block_header_proto_json,
            block_header_proto_metadata_bytes,
            operations_proto_metadata_bytes,
        }
    }
}

pub trait BlockStorageReader: Sync + Send {
    fn get(&self, block_hash: &BlockHash) -> Result<Option<BlockHeaderWithHash>, StorageError>;

    fn get_by_level(
        &self,
        head: &BlockHeaderWithHash,
        level: BlockLevel,
    ) -> Result<Option<BlockHeaderWithHash>, StorageError>;

    fn get_location(
        &self,
        block_hash: &BlockHash,
    ) -> Result<Option<BlockStorageColumnsLocation>, StorageError>;

    fn get_with_json_data(
        &self,
        block_hash: &BlockHash,
    ) -> Result<Option<(BlockHeaderWithHash, BlockJsonData)>, StorageError>;

    fn get_json_data(&self, block_hash: &BlockHash) -> Result<Option<BlockJsonData>, StorageError>;

    fn get_multiple_with_json_data(
        &self,
        block_hash: &BlockHash,
        limit: usize,
    ) -> Result<Vec<(BlockHeaderWithHash, BlockJsonData)>, StorageError>;

    fn get_multiple_without_json(
        &self,
        block_hash: &BlockHash,
        limit: usize,
    ) -> Result<Vec<BlockHeaderWithHash>, StorageError>;

    fn get_every_nth_with_json_data(
        &self,
        every_nth: u32,
        from_block_hash: &BlockHash,
        limit: usize,
    ) -> Result<Vec<(BlockHeaderWithHash, BlockJsonData)>, StorageError>;

    fn get_every_nth(
        &self,
        every_nth: u32,
        from_block_hash: &BlockHash,
        limit: usize,
    ) -> Result<Vec<BlockHeaderWithHash>, StorageError>;

    fn get_by_context_hash(
        &self,
        context_hash: &ContextHash,
    ) -> Result<Option<BlockHeaderWithHash>, StorageError>;

    fn contains_context_hash(&self, context_hash: &ContextHash) -> Result<bool, StorageError>;

    fn iterator(&self) -> Result<Vec<BlockHash>, StorageError>;
}

impl BlockStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            primary_index: BlockPrimaryIndex::new(persistent_storage.main_db()),
            predecessor_storage: PredecessorStorage::new(persistent_storage),
            by_context_hash_index: BlockByContextHashIndex::new(persistent_storage.main_db()),
            clog: persistent_storage.clog(),
        }
    }

    /// Stores header in key-value store and commit_log.
    /// If called multiple times for the same header, data are stored just first time.
    /// Returns true, if it is a new block
    pub fn put_block_header(
        &self,
        block_header: &BlockHeaderWithHash,
    ) -> Result<bool, StorageError> {
        if self.primary_index.contains(&block_header.hash)? {
            // we assume that, if primary_index contains hash, then also commit_log contains header data, header data cannot be change, so there is nothing to do
            return Ok(false);
        }

        self.clog
            .append(&BlockStorageColumn::BlockHeader(block_header.clone()))
            .map_err(StorageError::from)
            .and_then(|block_header_location| {
                let location = BlockStorageColumnsLocation {
                    block_header: block_header_location,
                    block_json_data: None,
                };
                self.primary_index
                    .put(&block_header.hash, &location)
                    .and(Ok(true))
            })
    }

    pub fn put_block_json_data(
        &self,
        block_hash: &BlockHash,
        json_data: BlockJsonData,
    ) -> Result<(), StorageError> {
        let updated_column_location = {
            let block_json_data_location = self
                .clog
                .append(&BlockStorageColumn::BlockJsonData(json_data))?;
            let mut column_location =
                self.primary_index
                    .get(block_hash)?
                    .ok_or_else(|| StorageError::MissingKey {
                        when: "put_block_json_data".into(),
                    })?;
            column_location.block_json_data = Some(block_json_data_location);
            column_location
        };
        let block_header = self.get_block_header_by_location(&updated_column_location)?;
        // update indexes
        self.primary_index
            .put(&block_header.hash, &updated_column_location)
    }

    pub fn assign_to_context(
        &self,
        block_hash: &BlockHash,
        context_hash: &ContextHash,
    ) -> Result<(), StorageError> {
        match self.primary_index.get(block_hash)? {
            Some(location) => self.by_context_hash_index.put(context_hash, &location),
            None => Err(StorageError::MissingKey {
                when: "assign_to_context".into(),
            }),
        }
    }

    pub fn store_predecessors(
        &self,
        block_hash: &BlockHash,
        direct_predecessor: &BlockHash,
    ) -> Result<(), StorageError> {
        self.predecessor_storage
            .store_predecessors(block_hash, direct_predecessor)?;
        Ok(())
    }

    #[inline]
    fn get_block_header_by_location(
        &self,
        location: &BlockStorageColumnsLocation,
    ) -> Result<BlockHeaderWithHash, StorageError> {
        match self
            .clog
            .get(&location.block_header)
            .map_err(StorageError::from)?
        {
            BlockStorageColumn::BlockHeader(block_header) => Ok(block_header),
            _ => Err(StorageError::InvalidColumn),
        }
    }

    #[inline]
    fn get_block_json_data_by_location(
        &self,
        location: &BlockStorageColumnsLocation,
    ) -> Result<Option<BlockJsonData>, StorageError> {
        match &location.block_json_data {
            Some(block_json_data_location) => match self
                .clog
                .get(block_json_data_location)
                .map_err(StorageError::from)?
            {
                BlockStorageColumn::BlockJsonData(json_data) => Ok(Some(json_data)),
                _ => Err(StorageError::InvalidColumn),
            },
            None => Ok(None),
        }
    }
}

impl BlockStorageReader for BlockStorage {
    #[inline]
    fn get(&self, block_hash: &BlockHash) -> Result<Option<BlockHeaderWithHash>, StorageError> {
        self.primary_index
            .get(block_hash)?
            .map(|location| self.get_block_header_by_location(&location))
            .transpose()
    }

    #[inline]
    fn get_by_level(
        &self,
        head: &BlockHeaderWithHash,
        level: BlockLevel,
    ) -> Result<Option<BlockHeaderWithHash>, StorageError> {
        let distance = head.header.level() - level;

        // distance cannot be negative
        if distance < 0 {
            return Err(StorageError::NegativeDistanceError);
        }

        if let Some(target_hash) =
            self.find_block_at_distance(head.hash.clone(), distance as u32)?
        {
            self.get(&target_hash)
        } else {
            Err(StorageError::PredecessorNotFound)
        }
    }

    #[inline]
    fn get_location(
        &self,
        block_hash: &BlockHash,
    ) -> Result<Option<BlockStorageColumnsLocation>, StorageError> {
        self.primary_index.get(block_hash)
    }

    #[inline]
    fn get_with_json_data(
        &self,
        block_hash: &BlockHash,
    ) -> Result<Option<(BlockHeaderWithHash, BlockJsonData)>, StorageError> {
        match self.primary_index.get(block_hash)? {
            Some(location) => self
                .get_block_json_data_by_location(&location)?
                .map(|json_data| {
                    self.get_block_header_by_location(&location)
                        .map(|block_header| (block_header, json_data))
                })
                .transpose(),
            None => Ok(None),
        }
    }

    #[inline]
    fn get_json_data(&self, block_hash: &BlockHash) -> Result<Option<BlockJsonData>, StorageError> {
        match self.primary_index.get(block_hash)? {
            Some(location) => self.get_block_json_data_by_location(&location),
            None => Ok(None),
        }
    }

    #[inline]
    fn get_multiple_with_json_data(
        &self,
        block_hash: &BlockHash,
        limit: usize,
    ) -> Result<Vec<(BlockHeaderWithHash, BlockJsonData)>, StorageError> {
        let mut res: Vec<(BlockHeaderWithHash, BlockJsonData)> = vec![];
        let mut hash = block_hash.clone();
        for _ in 0..limit {
            if let Some((block, json_data)) = self.get_with_json_data(&hash)? {
                res.push((block, json_data))
            } else {
                return Err(StorageError::MissingKey {
                    when: "get_multiple_with_json_data".into(),
                });
            }
            hash = if let Some(hash) = self.find_block_at_distance(hash, 1)? {
                hash
            } else {
                // if there are no more predecessors, we reached genesis
                break;
            }
        }

        Ok(res)
    }

    #[inline]
    fn get_every_nth_with_json_data(
        &self,
        every_nth: u32,
        from_block_hash: &BlockHash,
        limit: usize,
    ) -> Result<Vec<(BlockHeaderWithHash, BlockJsonData)>, StorageError> {
        let mut res: Vec<(BlockHeaderWithHash, BlockJsonData)> = vec![];
        let mut hash = from_block_hash.clone();
        for _ in 0..limit {
            if let Some((block, json_data)) = self.get_with_json_data(&hash)? {
                res.push((block, json_data))
            } else {
                return Err(StorageError::MissingKey {
                    when: "get_multiple_with_json_data".into(),
                });
            }
            hash = if let Some(hash) = self.find_block_at_distance(hash, every_nth)? {
                hash
            } else {
                // if there are no more predecessors, we reached genesis
                break;
            }
        }

        Ok(res)
    }

    #[inline]
    fn get_every_nth(
        &self,
        every_nth: u32,
        from_block_hash: &BlockHash,
        limit: usize,
    ) -> Result<Vec<BlockHeaderWithHash>, StorageError> {
        let mut res: Vec<BlockHeaderWithHash> = vec![];
        let mut hash = from_block_hash.clone();
        for _ in 0..limit {
            if let Some(block) = self.get(&hash)? {
                res.push(block)
            } else {
                return Err(StorageError::MissingKey {
                    when: "get_multiple_with_json_data".into(),
                });
            }
            hash = if let Some(hash) = self.find_block_at_distance(hash, every_nth)? {
                hash
            } else {
                // if there are no more predecessors, we reached genesis
                break;
            }
        }

        Ok(res)
    }

    #[inline]
    fn get_multiple_without_json(
        &self,
        block_hash: &BlockHash,
        limit: usize,
    ) -> Result<Vec<BlockHeaderWithHash>, StorageError> {
        let mut res: Vec<BlockHeaderWithHash> = vec![];
        let mut hash = block_hash.clone();
        for _ in 0..limit {
            if let Some(block) = self.get(&hash)? {
                res.push(block)
            } else {
                return Err(StorageError::MissingKey {
                    when: "get_multiple_with_json_data".into(),
                });
            }
            hash = if let Some(hash) = self.find_block_at_distance(hash, 1)? {
                hash
            } else {
                // if there are no more predecessors, we reached genesis
                break;
            }
        }

        Ok(res)
    }

    #[inline]
    fn get_by_context_hash(
        &self,
        context_hash: &ContextHash,
    ) -> Result<Option<BlockHeaderWithHash>, StorageError> {
        self.by_context_hash_index
            .get(context_hash)?
            .map(|location| self.get_block_header_by_location(&location))
            .transpose()
    }

    #[inline]
    fn contains_context_hash(&self, context_hash: &ContextHash) -> Result<bool, StorageError> {
        self.by_context_hash_index.contains(context_hash)
    }

    #[inline]
    fn iterator(&self) -> Result<Vec<BlockHash>, StorageError> {
        self.primary_index.iterator()
    }
}

impl PredecessorSearch for BlockStorage {
    fn get_predecessor_storage(&self) -> PredecessorStorage {
        self.predecessor_storage.clone()
    }
}

impl CommitLogSchema for BlockStorage {
    type Value = BlockStorageColumn;

    #[inline]
    fn name() -> &'static str {
        "block_storage"
    }
}

/// This mimics columns in a classic relational database.
#[derive(Serialize, Deserialize)]
pub enum BlockStorageColumn {
    BlockHeader(BlockHeaderWithHash),
    BlockJsonData(BlockJsonData),
}

impl BincodeEncoded for BlockStorageColumn {}

/// Holds reference to all stored columns.
#[derive(Serialize, Deserialize, Debug)]
pub struct BlockStorageColumnsLocation {
    pub block_header: Location,
    pub block_json_data: Option<Location>,
}

impl BincodeEncoded for BlockStorageColumnsLocation {}

/// Index block data as `block_header_hash -> location`.
#[derive(Clone)]
pub struct BlockPrimaryIndex {
    kv: Arc<BlockPrimaryIndexKV>,
}

pub type BlockPrimaryIndexKV = dyn TezedgeDatabaseWithIterator<BlockPrimaryIndex> + Sync + Send;

impl BlockPrimaryIndex {
    fn new(kv: Arc<BlockPrimaryIndexKV>) -> Self {
        Self { kv }
    }

    #[inline]
    fn put(
        &self,
        block_hash: &BlockHash,
        location: &BlockStorageColumnsLocation,
    ) -> Result<(), StorageError> {
        self.kv
            .put(block_hash, location)
            .map_err(StorageError::from)
    }

    #[inline]
    fn get(
        &self,
        block_hash: &BlockHash,
    ) -> Result<Option<BlockStorageColumnsLocation>, StorageError> {
        self.kv.get(block_hash).map_err(StorageError::from)
    }

    #[inline]
    fn contains(&self, block_hash: &BlockHash) -> Result<bool, StorageError> {
        self.kv.contains(block_hash).map_err(StorageError::from)
    }

    #[inline]
    fn iterator(&self) -> Result<Vec<BlockHash>, StorageError> {
        use crate::persistent::codec::Decoder;
        self.kv
            .find(IteratorMode::Start)?
            .map(|result| Ok(<Self as KeyValueSchema>::Key::decode(&result?.0)?))
            .collect()
    }
}

impl KeyValueSchema for BlockPrimaryIndex {
    type Key = BlockHash;
    type Value = BlockStorageColumnsLocation;
}

impl RocksDbKeyValueSchema for BlockPrimaryIndex {
    #[inline]
    fn name() -> &'static str {
        "block_storage"
    }
}

impl KVStoreKeyValueSchema for BlockPrimaryIndex {
    fn column_name() -> &'static str {
        Self::name()
    }
}

/// Index block data as `level -> location`.
#[derive(Clone)]
pub struct BlockByContextHashIndex {
    kv: Arc<BlockByContextHashIndexKV>,
}

pub type BlockByContextHashIndexKV =
    dyn TezedgeDatabaseWithIterator<BlockByContextHashIndex> + Sync + Send;

impl BlockByContextHashIndex {
    fn new(kv: Arc<BlockByContextHashIndexKV>) -> Self {
        Self { kv }
    }

    fn put(
        &self,
        context_hash: &ContextHash,
        location: &BlockStorageColumnsLocation,
    ) -> Result<(), StorageError> {
        self.kv
            .put(context_hash, location)
            .map_err(StorageError::from)
    }

    fn get(
        &self,
        context_hash: &ContextHash,
    ) -> Result<Option<BlockStorageColumnsLocation>, StorageError> {
        self.kv.get(context_hash).map_err(StorageError::from)
    }

    fn contains(&self, context_hash: &ContextHash) -> Result<bool, StorageError> {
        self.kv.contains(context_hash).map_err(StorageError::from)
    }
}

impl KeyValueSchema for BlockByContextHashIndex {
    type Key = ContextHash;
    type Value = BlockStorageColumnsLocation;
}

impl RocksDbKeyValueSchema for BlockByContextHashIndex {
    #[inline]
    fn name() -> &'static str {
        "block_by_context_hash_storage"
    }
}

impl KVStoreKeyValueSchema for BlockByContextHashIndex {
    fn column_name() -> &'static str {
        Self::name()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use anyhow::Error;
    use tezos_messages::p2p::encoding::block_header::BlockHeader;

    use tests_common::TmpStorage;

    use crate::tests_common;
    use crate::BlockStorageReader;

    use super::*;
    use tezos_messages::p2p::binary_message::BinaryRead;

    fn mocked_block_storage(path: &str) -> Result<(Vec<BlockHeaderWithHash>, BlockStorage), Error> {
        let headers_str = vec![
            ("BMPpsdYyqRPUx4bPM187UguF29NUAxNhh5tXXrSNMbb6UKZf2iM", "00000001008fcf233671b6a04fcf679d2a381c2544ea6c1ea29ba6157776ed8424418da9c100000000000000060176f52f440c8e4ab99a2a4ef76474c4bcf4ee09238150ed77fd73fb3be1fe4af30000000c0000000800000000000000016ae39b9a0de0eff1d52ae16a9282747edf61f89e4bc72b943221ca94fd3b9b48000000024131"),
            ("BLq4AAun3eFcHWA8BJZgnUy6DEkSRZde69MziAh2ct4wCE1weCY", "0000000200dd7c2bbc9594cd39fe49f29637f6bff5ca9d50f11dd341508bfacf714b7e28e300000000000000100107eb8155a0827d5ef666e43b15acdea8adb58e3817aa6ca7ac4a9c55eb41ebd20000000c000000080000000000000002514aa7bfad26b4289f9225f722803ad5d62294c931e8ccd8d237ea7e87afb236000000024132"),
            ("BLMw95k8rwLf2aZmGiRh9jFMuKGgQXipKUMHk4WLE8YM4Z2WJ86", "000000030093131926e0640848ba856c0dd8efd864f7d9a0b5c263a5e03f869f5dc598943b000000000000001801fc2bb2c1c5997df6928a6dc2099f44125464dd8dc38fcf24b67db2a79fc82c0f0000000c000000080000000000000003f1acd04bbdb7838be6b8f74d74963cbb6b89633114e986ae1674eb80f0a6420c000000024133"),
            ("BME4s6XySdprEvUwyG9A5YvcgsDnKzBKKSVQ6UoZ6mk3rBR7jNt", "0000000400557e36cf1cae93bda2831222102b7c4ce19a131c3d3d01aec176a7b00d1ee98a000000000000001a01db7daffe2c9e790d589a6b3ce96dc49efbb10fbb50f1e636707f751a5ae1f30b0000000c000000080000000000000004109c7826a2c620d2c5cac965b853a9bc98d6385ace6601a3a1515d518143f9af000000024134"),
            ("BKjP9MgvzqEDCdXaVvy2Yt5C52UtpUa9puUQ6konopdi9UKFFWp", "0000000500c7539814cb12b4fc92c3768ad7dc7322e6abeb3b9b563b556ecb69d341b260040000000000000022019cc9c998792c4ef6eb7c797cc8aa6c3dddd360d611746a3e827fc1dbecb5fe850000000c00000008000000000000000560bb24813522ffed0aabe1b776e9a2317639e4c57cf51ceb4145d8d75976447f000000024135"),
            ("BKm9XUEw7yWXEF8ERb9QQXncRe44smMJy8hgkpE57Pv2U6mLNtn", "0000000600027f8016df677b17a3b62d9f4e34ce9544289554a546ce5d1db87cbe2a4f927b00000000000000230184569a705486fd7bb7231ae792a28a499d94be7240d69671a5bc72ad1d1b65b60000000c00000008000000000000000669497732182ffb3e538a9e25c1443b7d1a989daae42e435edcd3ff3ef2ef7d12000000024136"),
        ];

        let blocks_in_mem: Vec<BlockHeaderWithHash> = headers_str
            .iter()
            .map(|(_, header_str)| {
                let block_header: BlockHeader = BlockHeader::from_bytes(
                    hex::decode(header_str).expect("Failed to decode hex header"),
                )
                .expect("Failed to deserialze header");

                BlockHeaderWithHash::new(block_header)
                    .expect("Failed to create BlockHeaderWithHash")
            })
            .collect();

        if Path::new(path).exists() {
            std::fs::remove_dir_all(path).unwrap();
        }

        let tmp_storage =
            TmpStorage::create_to_out_dir(path).expect("failed to create tmp storage");

        let block_storage = BlockStorage::new(tmp_storage.storage());

        // initialize storage with a few blocks
        let mut predecessor_hash: BlockHash = blocks_in_mem[0].hash.clone();
        for block in blocks_in_mem.iter() {
            block_storage
                .put_block_header(block)
                .expect("Failed to store block");
            block_storage
                .put_block_json_data(
                    &block.hash,
                    BlockJsonData::new("".to_string(), vec![], vec![]),
                )
                .expect("Failed to store block json data");
            block_storage
                .store_predecessors(&block.hash, &predecessor_hash)
                .expect("Failed to store predecessors");
            predecessor_hash = block.hash.clone();
        }

        Ok((blocks_in_mem, block_storage))
    }

    #[test]
    fn block_storage_reader_get_test() -> Result<(), Error> {
        let path = "__block_storage_reader_get";
        {
            let (blocks_in_mem, block_storage) = mocked_block_storage(path)?;

            // test geting by block hash
            println!("\nTesting get");
            for block in blocks_in_mem.iter() {
                println!("Testing for block hash: {}", block.hash.to_base58_check());
                assert_eq!(
                    block.hash,
                    block_storage
                        .get(&block.hash)
                        .expect("Failed to get block")
                        .unwrap()
                        .hash
                );
            }
        }
        Ok(())
    }

    #[test]
    fn block_storage_reader_get_by_level_test() -> Result<(), Error> {
        let path = "__block_storage_reader_get_by_level";
        {
            let (blocks_in_mem, block_storage) = mocked_block_storage(path)?;

            println!("\nTesting get_by_level");
            // test getting by level
            for i in 1..=6 {
                println!("Testing for level : {}", i);
                assert_eq!(
                    // Note: level 1 is stored as on index 0
                    blocks_in_mem[i - 1],
                    block_storage
                        .get_by_level(blocks_in_mem.last().unwrap(), i as i32)
                        .expect("Failed to get block")
                        .unwrap()
                );
            }
        }
        Ok(())
    }

    #[test]
    fn block_storage_reader_get_multiple_with_json_data_test() -> Result<(), Error> {
        let path = "__block_storage_reader_get_multiple_with_json_data";
        {
            let (blocks_in_mem, block_storage) = mocked_block_storage(path)?;

            // test get multiple with json data
            println!("\nTesting get_multiple_with_json_data with limit 2 from head");
            assert_eq!(
                vec![blocks_in_mem[5].hash.clone(), blocks_in_mem[4].hash.clone()],
                block_storage
                    .get_multiple_with_json_data(&blocks_in_mem.last().unwrap().hash, 2)
                    .expect("Faled to get_multiple_with_json_data")
                    .iter()
                    .map(|(b, _)| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );

            // test get multiple with json data
            println!("\nTesting get_multiple_with_json_data with limit 6 from head (all blocks should be recieved)");
            assert_eq!(
                blocks_in_mem
                    .iter()
                    .rev()
                    .map(|v| v.hash.clone())
                    .collect::<Vec<BlockHash>>(),
                block_storage
                    .get_multiple_with_json_data(&blocks_in_mem.last().unwrap().hash, 6)
                    .expect("Faled to get_multiple_with_json_data")
                    .iter()
                    .map(|(b, _)| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );

            // test get multiple with json data
            println!("\nTesting get_multiple_with_json_data with limit greater than the count of the predecessors from head (all blocks should be recieved)");
            assert_eq!(
                blocks_in_mem
                    .iter()
                    .rev()
                    .map(|v| v.hash.clone())
                    .collect::<Vec<BlockHash>>(),
                block_storage
                    .get_multiple_with_json_data(&blocks_in_mem.last().unwrap().hash, 10)
                    .expect("Faled to get_multiple_with_json_data")
                    .iter()
                    .map(|(b, _)| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );
        }
        Ok(())
    }

    #[test]
    fn block_storage_reader_get_every_nth_test() -> Result<(), Error> {
        let path = "__block_storage_reader_get_every_nth_data";

        {
            let (blocks_in_mem, block_storage) = mocked_block_storage(path)?;

            // test every_nth
            println!("\nTesting get_every_nth: 2 limit: 10");
            assert_eq!(
                vec![
                    blocks_in_mem[5].hash.clone(),
                    blocks_in_mem[3].hash.clone(),
                    blocks_in_mem[1].hash.clone()
                ],
                block_storage
                    .get_every_nth(2, &blocks_in_mem.last().unwrap().hash, 10)
                    .expect("Faled to get_every_nth")
                    .iter()
                    .map(|b| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );

            // test every_nth
            println!("\nTesting get_every_nth: 1 limit: 10");
            assert_eq!(
                blocks_in_mem
                    .iter()
                    .rev()
                    .map(|v| v.hash.clone())
                    .collect::<Vec<BlockHash>>(),
                block_storage
                    .get_every_nth(1, &blocks_in_mem.last().unwrap().hash, 10)
                    .expect("Faled to get_every_nth")
                    .iter()
                    .map(|b| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );

            // test every_nth
            println!("\nTesting get_every_nth: 3 limit: 10");
            assert_eq!(
                vec![blocks_in_mem[5].hash.clone(), blocks_in_mem[2].hash.clone(),],
                block_storage
                    .get_every_nth(3, &blocks_in_mem.last().unwrap().hash, 10)
                    .expect("Faled to get_every_nth")
                    .iter()
                    .map(|b| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );

            // test every_nth
            println!("\nTesting get_every_nth: 2 limit: 1");
            assert_eq!(
                vec![blocks_in_mem[5].hash.clone(),],
                block_storage
                    .get_every_nth(2, &blocks_in_mem.last().unwrap().hash, 1)
                    .expect("Faled to get_every_nth")
                    .iter()
                    .map(|b| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );

            // test every_nth where the step is too big
            println!("\nTesting get_every_nth with step too big");
            assert_eq!(
                vec![blocks_in_mem[5].hash.clone(),],
                block_storage
                    .get_every_nth(10, &blocks_in_mem.last().unwrap().hash, 10)
                    .expect("Faled to get_every_nth")
                    .iter()
                    .map(|b| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );
        }
        Ok(())
    }

    #[test]
    fn block_storage_reader_get_multiple_without_json_test() -> Result<(), Error> {
        let path = "__block_storage_reader_get_multiple_without_json_test";

        {
            let (blocks_in_mem, block_storage) = mocked_block_storage(path)?;

            // test get multiple with json data
            println!("\nTesting get_multiple_without_json with limit 2 from head");
            assert_eq!(
                vec![blocks_in_mem[5].hash.clone(), blocks_in_mem[4].hash.clone()],
                block_storage
                    .get_multiple_without_json(&blocks_in_mem.last().unwrap().hash, 2)
                    .expect("Faled to get_multiple_without_json")
                    .iter()
                    .map(|b| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );

            // test get multiple with json data
            println!("\nTesting get_multiple_without_json with limit 6 from head (all blocks should be recieved)");
            assert_eq!(
                blocks_in_mem
                    .iter()
                    .rev()
                    .map(|v| v.hash.clone())
                    .collect::<Vec<BlockHash>>(),
                block_storage
                    .get_multiple_without_json(&blocks_in_mem.last().unwrap().hash, 6)
                    .expect("Faled to get_multiple_without_json")
                    .iter()
                    .map(|b| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );

            // test get multiple with json data
            println!("\nTesting get_multiple_without_json with limit greater than the count of the predecessors from head (all blocks should be recieved)");
            assert_eq!(
                blocks_in_mem
                    .iter()
                    .rev()
                    .map(|v| v.hash.clone())
                    .collect::<Vec<BlockHash>>(),
                block_storage
                    .get_multiple_without_json(&blocks_in_mem.last().unwrap().hash, 10)
                    .expect("Faled to get_multiple_without_json")
                    .iter()
                    .map(|b| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );
        }
        Ok(())
    }

    #[test]
    fn block_storage_reader_get_every_nth_with_json_data_test() -> Result<(), Error> {
        let path = "__block_storage_reader_get_every_nth_with_json_data_test";

        {
            let (blocks_in_mem, block_storage) = mocked_block_storage(path)?;

            // test every_nth
            println!("\nTesting get_every_nth_with_json_data: 2 limit: 10");
            assert_eq!(
                vec![
                    blocks_in_mem[5].hash.clone(),
                    blocks_in_mem[3].hash.clone(),
                    blocks_in_mem[1].hash.clone()
                ],
                block_storage
                    .get_every_nth_with_json_data(2, &blocks_in_mem.last().unwrap().hash, 10)
                    .expect("Faled to get_every_nth_with_json_data")
                    .iter()
                    .map(|(b, _)| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );

            // test every_nth
            println!("\nTesting get_every_nth_with_json_data: 1 limit: 10");
            assert_eq!(
                blocks_in_mem
                    .iter()
                    .rev()
                    .map(|v| v.hash.clone())
                    .collect::<Vec<BlockHash>>(),
                block_storage
                    .get_every_nth_with_json_data(1, &blocks_in_mem.last().unwrap().hash, 10)
                    .expect("Faled to get_every_nth_with_json_data")
                    .iter()
                    .map(|(b, _)| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );

            // test every_nth
            println!("\nTesting get_every_nth_with_json_data: 3 limit: 10");
            assert_eq!(
                vec![blocks_in_mem[5].hash.clone(), blocks_in_mem[2].hash.clone(),],
                block_storage
                    .get_every_nth_with_json_data(3, &blocks_in_mem.last().unwrap().hash, 10)
                    .expect("Faled to get_every_nth_with_json_data")
                    .iter()
                    .map(|(b, _)| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );

            // test every_nth
            println!("\nTesting get_every_nth_with_json_data: 2 limit: 1");
            assert_eq!(
                vec![blocks_in_mem[5].hash.clone(),],
                block_storage
                    .get_every_nth_with_json_data(2, &blocks_in_mem.last().unwrap().hash, 1)
                    .expect("Faled to get_every_nth_with_json_data")
                    .iter()
                    .map(|(b, _)| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );

            // test every_nth where the step is too big
            println!("\nTesting get_every_nth_with_json_data with step too big");
            assert_eq!(
                vec![blocks_in_mem[5].hash.clone(),],
                block_storage
                    .get_every_nth_with_json_data(10, &blocks_in_mem.last().unwrap().hash, 10)
                    .expect("Faled to get_every_nth_with_json_data")
                    .iter()
                    .map(|(b, _)| b.hash.clone())
                    .collect::<Vec<BlockHash>>()
            );
        }
        Ok(())
    }
}
