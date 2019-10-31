use super::base_types::*;
use serde::{Serialize, Deserialize};
use tezos_messages::p2p::encoding::prelude::*;
use crate::helpers::{FullBlockInfo, InnerBlockHeader};

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
    header: InnerBlockHeader,
    metadata: (),
    operations: Vec<OperationsForBlocksMessage>,
}

impl From<FullBlockInfo> for BlockInfo {
    fn from(val: FullBlockInfo) -> Self {
        Self {
            protocol: None,
            chain_id: None,
            hash: Some(val.hash.into()),
            header: val.header,
            operations: val.operations,
            metadata: (),
        }
    }
}