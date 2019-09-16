use std::sync::Arc;

use failure::Fail;

pub use block_state::BlockState;
use networking::p2p::binary_message::{BinaryMessage, MessageHash, MessageHashError};
use networking::p2p::encoding::prelude::BlockHeader;
pub use operations_state::{MissingOperations, OperationsState};
use tezos_encoding::hash::{HashRef, ToHashRef};

use crate::persistent::{Codec, DBError, SchemaError};

pub mod persistent;
pub mod block_storage;
pub mod operations_storage;
pub mod operations_state;
pub mod block_state;


#[derive(Clone, PartialEq, Debug)]
pub struct BlockHeaderWithHash {
    pub header: Arc<BlockHeader>,
    pub hash: HashRef,
}

impl BlockHeaderWithHash {
    pub fn new(block_header: BlockHeader) -> Result<Self, MessageHashError> {
        Ok(BlockHeaderWithHash {
            hash: block_header.message_hash()?.to_hash_ref(),
            header: Arc::new(block_header),
        })
    }
}

impl Codec for BlockHeaderWithHash {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        bincode::deserialize(bytes)
            .map_err(|_| SchemaError::DecodeError)
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        bincode::serialize(self)
            .map_err(|_| SchemaError::DecodeError)
    }
}

/// Possible errors for storage
#[derive(Debug, Fail)]
pub enum StorageError {
    #[fail(display = "Database error: {}", error)]
    DBError {
        error: DBError
    },
    #[fail(display = "Key is missing in storage")]
    MissingKey,
    #[fail(display = "Unexpected value found in storage")]
    UnexpectedValue
}

impl From<DBError> for StorageError {
    fn from(error: DBError) -> Self {
        StorageError::DBError { error }
    }
}

#[cfg(test)]
mod tests {
    use std::iter::FromIterator;

    use failure::Error;

    use tezos_encoding::hash::{HashEncoding, HashType};

    use super::*;

    #[test]
    fn block_header_with_hash_encoded_equals_decoded() -> Result<(), Error> {
        let expected = BlockHeaderWithHash {
            hash: HashRef::new(HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?),
            header: Arc::new(BlockHeader {
                level: 34,
                proto: 1,
                predecessor: HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?,
                timestamp: 5635634,
                validation_pass: 4,
                operations_hash: HashEncoding::new(HashType::OperationListListHash).string_to_bytes("LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc")?,
                fitness: vec![vec![0, 0]],
                context: HashEncoding::new(HashType::ContextHash).string_to_bytes("CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd")?,
                protocol_data: vec![0, 1, 2, 3, 4, 5, 6, 7, 8]
            })
        };
        let encoded_bytes = expected.encode()?;
        let decoded = BlockHeaderWithHash::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }
}
