use std::rc::Rc;
use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};
use tezos_encoding::hash::{HashEncoding, Prefix};

use super::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockHeader {
    level: i32,
    proto: u8,
    predecessor: BlockHash,
    timestamp: i64,
    validation_pass: u8,
    operations_hash: Vec<u8>,
    fitness: Vec<Vec<u8>>,
    context: Vec<u8>,
    protocol_data: Vec<u8>
}

#[allow(dead_code)]
impl BlockHeader {
    pub fn get_level(&self) -> i32 { self.level}
    pub fn get_proto(&self) -> u8 { self.proto }
    pub fn get_predecessor(&self) -> &BlockHash { &self.predecessor }
    pub fn get_timestamp(&self) -> i64 { self.timestamp }
    pub fn get_validation_pass(&self) -> u8 { self.validation_pass }
    pub fn get_operations_hash(&self) -> &Vec<u8> { &self.operations_hash }
    pub fn get_fitness(&self) -> &Vec<Vec<u8>> { &self.fitness }
    pub fn get_context(&self) -> &Vec<u8> { &self.context }
    pub fn get_protocol_data(&self) -> &Vec<u8> { &self.protocol_data }
}

impl HasEncoding for BlockHeader {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("level", Encoding::Int32),
            Field::new("proto", Encoding::Uint8),
            Field::new("predecessor", Encoding::Hash(HashEncoding::new(32, Prefix::BlockHash))),
            Field::new("timestamp", Encoding::Timestamp),
            Field::new("validation_pass", Encoding::Uint8),
            Field::new("operations_hash", Encoding::Hash(HashEncoding::new(32, Prefix::OperationListListHash))),
            Field::new("fitness", Encoding::Split(Rc::new(|schema_type|
                match schema_type {
                    SchemaType::Json => Encoding::dynamic(Encoding::list(Encoding::Bytes)),
                    SchemaType::Binary => Encoding::dynamic(Encoding::list(
                        Encoding::dynamic(Encoding::list(Encoding::Uint8))
                    ))
                }
            ))),
            Field::new("context", Encoding::Hash(HashEncoding::new(32, Prefix::ContextHash))),
            Field::new("protocol_data", Encoding::Split(Rc::new(|schema_type|
                match schema_type {
                    SchemaType::Json => Encoding::Bytes,
                    SchemaType::Binary => Encoding::list(Encoding::Uint8)
                }
            )))
        ])
    }
}

#[cfg(test)]
mod tests {
    use failure::Error;
    use crate::p2p::message::BinaryMessage;
    use super::*;

    #[test]
    fn can_deserialize() -> Result<(), Error> {
        let message_bytes = hex::decode("00006d6e0102dd00defaf70c53e180ea148b349a6feb4795610b2abc7b07fe91ce50a90814000000005c1276780432bc1d3a28df9a67b363aa1638f807214bb8987e5f9c0abcbd69531facffd1c80000001100000001000000000800000000000c15ef15a6f54021cb353780e2847fb9c546f1d72c1dc17c3db510f45553ce501ce1de000000000003c762c7df00a856b8bfcaf0676f069f825ca75f37f2bee9fe55ba109cec3d1d041d8c03519626c0c0faa557e778cb09d2e0c729e8556ed6a7a518c84982d1f2682bc6aa753f")?;
        let block_header = BlockHeader::from_bytes(message_bytes)?;
        assert_eq!(28014, block_header.get_level());
        assert_eq!(1, block_header.get_proto());
        assert_eq!(4, block_header.get_validation_pass());
        assert_eq!(2, block_header.get_fitness().len());
        assert_eq!("000000000003c762c7df00a856b8bfcaf0676f069f825ca75f37f2bee9fe55ba109cec3d1d041d8c03519626c0c0faa557e778cb09d2e0c729e8556ed6a7a518c84982d1f2682bc6aa753f", &hex::encode(&block_header.get_protocol_data()));

        Ok(())
    }
}