use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};

use super::*;

#[derive(Serialize, Deserialize, Debug)]
pub struct GetCurrentBranchMessage {
    chain_id: ChainId,
}

impl GetCurrentBranchMessage {
    pub fn new(chain_id: ChainId) -> Self {
        GetCurrentBranchMessage { chain_id }
    }

    pub fn get_chain_id(&self) -> &ChainId {
        &self.chain_id
    }
}

impl HasEncoding for GetCurrentBranchMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("chain_id", Encoding::sized(4, Encoding::Bytes))
        ])
    }
}