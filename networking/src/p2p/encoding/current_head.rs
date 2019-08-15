use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::hash::{HashEncoding, HashType};

use super::*;
use super::block_header::BlockHeader;
use super::mempool::Mempool;

#[derive(Serialize, Deserialize, Debug)]
pub struct CurrentHeadMessage {
    chain_id: ChainId,
    current_block_header: BlockHeader,
    current_mempool: Mempool
}

#[allow(dead_code)]
impl CurrentHeadMessage {

    pub fn get_chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    pub fn get_current_block_header(&self) -> &BlockHeader {
        &self.current_block_header
    }

    pub fn get_current_mempool(&self) -> &Mempool {
        &self.current_mempool
    }
}

impl HasEncoding for CurrentHeadMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("chain_id", Encoding::Hash(HashEncoding::new(HashType::ChainId))),
            Field::new("current_block_header", Encoding::dynamic(BlockHeader::encoding())),
            Field::new("current_mempool", Mempool::encoding())
        ])
    }
}