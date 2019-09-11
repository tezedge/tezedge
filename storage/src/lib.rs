use networking::p2p::binary_message::{MessageHash, MessageHashError};
use networking::p2p::encoding::prelude::BlockHeader;
use tezos_encoding::hash::{HashRef, ToHashRef};
use std::sync::Arc;

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
