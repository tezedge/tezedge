use networking::p2p::binary_message::{MessageHash, MessageHashError};
use networking::p2p::encoding::prelude::BlockHeader;
use tezos_encoding::hash::{HashRef, ToHashRef};
use std::sync::Arc;

//mod persistent;
mod block_storage;
mod operations_storage;
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

pub use block_state::BlockState;
pub use operations_state::{OperationsState, MissingOperations};
