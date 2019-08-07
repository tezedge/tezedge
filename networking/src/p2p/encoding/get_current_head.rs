use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::hash;
use tezos_encoding::hash::HashEncoding;

use crate::p2p::encoding::ChainId;

#[derive(Serialize, Deserialize, Debug)]
pub struct GetCurrentHeadMessage {
    chain_id: ChainId,
}

impl GetCurrentHeadMessage {
    #[allow(dead_code)]
    pub fn new(chain_id: ChainId) -> Self {
        GetCurrentHeadMessage { chain_id }
    }
}

impl HasEncoding for GetCurrentHeadMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("chain_id", Encoding::Hash(HashEncoding::new(4, &hash::prefix::CHAIN_ID)))
        ])
    }
}
