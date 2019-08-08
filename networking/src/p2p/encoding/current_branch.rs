use std::rc::Rc;

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};
use tezos_encoding::hash;
use tezos_encoding::hash::HashEncoding;

use crate::p2p::encoding::block_header::BlockHeader;

use super::*;

#[derive(Serialize, Deserialize, Debug)]
pub struct CurrentBranchMessage {
    chain_id: ChainId,
    current_branch: CurrentBranch,
}

impl CurrentBranchMessage {
    pub fn get_chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    pub fn get_current_branch(&self) -> &CurrentBranch {
        &self.current_branch
    }
}

impl HasEncoding for CurrentBranchMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("chain_id", Encoding::Hash(HashEncoding::new(4, &hash::prefix::CHAIN_ID))),
            Field::new("current_branch", CurrentBranch::encoding())
        ])
    }
}

// -----------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug)]
pub struct CurrentBranch {
    current_head: BlockHeader,
    history: Vec<u8>,
}

impl CurrentBranch {

    pub fn get_current_head(&self) -> &BlockHeader {
        &self.current_head
    }

    #[allow(dead_code)]
    pub fn get_history(&self) -> &Vec<u8> {
        &self.history
    }
}

impl HasEncoding for CurrentBranch {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("current_head", BlockHeader::encoding()),
            Field::new("history", Encoding::Split(Rc::new(|schema_type|
                match schema_type {
                    SchemaType::Json => Encoding::Unit, // TODO: decode as list of hashes when history is needed
                    SchemaType::Binary => Encoding::list(Encoding::Uint8)
                }
            )))
        ])
    }
}