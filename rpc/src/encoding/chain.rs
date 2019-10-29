use super::base_types::*;
use serde::{Serialize, Deserialize};
use tezos_messages::p2p::encoding::prelude::*;
use std::sync::Arc;
use crate::helpers::FullBlockInfo;
use tezos_encoding::hash::{HashEncoding, HashType};

// { protocol: ProtocolHash,
//   chain_id: ChainHash,
//   hash: BlockHash,
//   header: BlockHeader,
//   metadata: BlockMetadata,
//   operations: Vec<BlockOperations> }
#[derive(Serialize, Deserialize, Debug)]
pub struct BlockInfo {
    protocol: Option<UniString>,
    chain_id: Option<UniString>,
    hash: Option<UniString>,
    header: Arc<BlockHeader>,
    metadata: (),
    operations: Vec<OperationsForBlocksMessage>,
}

impl From<FullBlockInfo> for BlockInfo {
    fn from(val: FullBlockInfo) -> Self {
        let block_hash = Some(HashEncoding::new(HashType::BlockHash).bytes_to_string(&val.header.hash).into());
        Self {
            protocol: None,
            chain_id: None,
            hash: block_hash,
            header: val.header.header,
            operations: val.operations,
            metadata: (),
        }
    }
}