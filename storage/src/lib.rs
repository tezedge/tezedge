use std::sync::Arc;

pub use block_state::BlockState;
use networking::p2p::binary_message::{MessageHash, MessageHashError, BinaryMessage};
use networking::p2p::encoding::prelude::BlockHeader;
pub use operations_state::{MissingOperations, OperationsState};
use tezos_encoding::hash::{HashRef, ToHashRef};

use crate::persistent::{Codec, SchemaError};

pub mod persistent;
pub mod block_storage;
pub mod operations_storage;
pub mod operations_state;
pub mod block_state;


#[derive(Clone)]
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
        let block_header = BlockHeader::from_bytes(bytes.to_vec())
            .map_err(|_| SchemaError::DecodeError)?;

        BlockHeaderWithHash::new(block_header)
            .map_err(|_| SchemaError::DecodeError)
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        self.header.as_bytes()
            .map_err(|_| SchemaError::EncodeError)
    }
}

