use std::rc::Rc;

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};
use tezos_encoding::hash::{BlockHash, ContextHash, HashEncoding, HashType, OperationListListHash};


#[derive(Serialize, Deserialize, Debug)]
pub struct BlockHeaderMessage {
    pub block_header: BlockHeader
}

impl HasEncoding for BlockHeaderMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("block_header", Encoding::dynamic(BlockHeader::encoding())),
        ])
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug)]
pub struct GetBlockHeadersMessage {
    get_block_headers: Vec<BlockHash>,
}

impl GetBlockHeadersMessage {
    pub fn get_block_headers(&self) -> &Vec<BlockHash> {
        &self.get_block_headers
    }
}

impl HasEncoding for GetBlockHeadersMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("get_block_headers", Encoding::dynamic(Encoding::list(Encoding::Hash(HashEncoding::new(HashType::BlockHash))))),
        ])
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockHeader {
    level: i32,
    proto: u8,
    predecessor: BlockHash,
    timestamp: i64,
    validation_pass: u8,
    operations_hash: OperationListListHash,
    fitness: Vec<Vec<u8>>,
    context: ContextHash,
    protocol_data: Vec<u8>
}

#[allow(dead_code)]
impl BlockHeader {
    pub fn get_level(&self) -> i32 { self.level}
    pub fn get_proto(&self) -> u8 { self.proto }
    pub fn get_predecessor(&self) -> &BlockHash { &self.predecessor }
    pub fn get_timestamp(&self) -> i64 { self.timestamp }
    pub fn get_validation_pass(&self) -> u8 { self.validation_pass }
    pub fn get_operations_hash(&self) -> &OperationListListHash { &self.operations_hash }
    pub fn get_fitness(&self) -> &Vec<Vec<u8>> { &self.fitness }
    pub fn get_context(&self) -> &ContextHash { &self.context }
    pub fn get_protocol_data(&self) -> &Vec<u8> { &self.protocol_data }
}

impl HasEncoding for BlockHeader {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("level", Encoding::Int32),
            Field::new("proto", Encoding::Uint8),
            Field::new("predecessor", Encoding::Hash(HashEncoding::new(HashType::BlockHash))),
            Field::new("timestamp", Encoding::Timestamp),
            Field::new("validation_pass", Encoding::Uint8),
            Field::new("operations_hash", Encoding::Hash(HashEncoding::new(HashType::OperationListListHash))),
            Field::new("fitness", Encoding::Split(Rc::new(|schema_type|
                match schema_type {
                    SchemaType::Json => Encoding::dynamic(Encoding::list(Encoding::Bytes)),
                    SchemaType::Binary => Encoding::dynamic(Encoding::list(
                        Encoding::dynamic(Encoding::list(Encoding::Uint8))
                    ))
                }
            ))),
            Field::new("context", Encoding::Hash(HashEncoding::new(HashType::ContextHash))),
            Field::new("protocol_data", Encoding::Split(Rc::new(|schema_type|
                match schema_type {
                    SchemaType::Json => Encoding::Bytes,
                    SchemaType::Binary => Encoding::list(Encoding::Uint8)
                }
            )))
        ])
    }
}