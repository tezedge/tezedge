use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::hash::{ChainId, HashEncoding, HashType};

use super::block_header::BlockHeader;
use super::mempool::Mempool;

#[derive(Serialize, Deserialize, Debug)]
pub struct CurrentHeadMessage {
    pub chain_id: ChainId,
    pub current_block_header: BlockHeader,
    pub current_mempool: Mempool
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

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug)]
pub struct GetCurrentHeadMessage {
    pub chain_id: ChainId,
}

impl HasEncoding for GetCurrentHeadMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("chain_id", Encoding::Hash(HashEncoding::new(HashType::ChainId)))
        ])
    }
}
