use super::base_types::*;
use serde::{Serialize, Deserialize};
use tezos_messages::p2p::encoding::prelude::*;
use crate::helpers::FullBlockInfo;
use tezos_encoding::hash::{HashEncoding, HashType};
use crate::ts_to_rfc3339;

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
        let block_hash = Some(HashEncoding::new(HashType::BlockHash).bytes_to_string(&val.header.hash).into());
        Self {
            protocol: None,
            chain_id: None,
            hash: block_hash,
            header: val.header.header.into(),
            operations: val.operations,
            metadata: (),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InnerBlockHeader {
    level: i32,
    proto: u8,
    predecessor: UniString,
    timestamp: TimeStamp,
    validation_pass: u8,
    operations_hash: UniString,
    fitness: Vec<UniString>,
    context: UniString,
    priority: i32,
    proof_of_work_nonce: UniString,
    signature: UniString,
}

impl<T: AsRef<BlockHeader>> From<T> for InnerBlockHeader {
    fn from(val: T) -> Self {
        let val = val.as_ref();
        Self {
            level: val.level(),
            proto: val.proto(),
            predecessor: HashEncoding::new(HashType::BlockHash).bytes_to_string(&val.predecessor()).into(),
            timestamp: TimeStamp::Rfc(ts_to_rfc3339(val.timestamp())),
            validation_pass: val.validation_pass(),
            operations_hash: HashEncoding::new(HashType::OperationListListHash).bytes_to_string(&val.operations_hash()).into(),
            fitness: val.fitness().iter().map(|x| hex::encode(&x).into()).collect(),
            context: HashEncoding::new(HashType::ContextHash).bytes_to_string(&val.context()).into(),
            priority: 0,
            proof_of_work_nonce: "".into(),
            signature: "".into(),
        }
    }
}