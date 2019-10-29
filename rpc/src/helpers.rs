use storage::BlockHeaderWithHash;
use tezos_messages::p2p::encoding::prelude::*;

pub type BlockMeta = ();

#[derive(Debug, Clone)]
pub struct FullBlockInfo {
    pub header: BlockHeaderWithHash,
    pub metadata: BlockMeta,
    pub operations: Vec<OperationsForBlocksMessage>,
}