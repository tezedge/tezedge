// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};
use tezos_encoding::hash::{BlockHash, HashEncoding, HashType, OperationHash};

#[derive(Serialize, Deserialize, Debug)]
pub struct OperationMessage {
    operation: Operation
}

impl HasEncoding for OperationMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("operation", Operation::encoding())
        ])
    }
}


// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Operation {
    branch: BlockHash,
    data: Vec<u8>,
}

impl Operation {
    pub fn branch(&self) -> &BlockHash {
        &self.branch
    }

    pub fn data(&self) -> &Vec<u8> {
        &self.data
    }
}

impl HasEncoding for Operation {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("branch", Encoding::Hash(HashEncoding::new(HashType::BlockHash))),
            Field::new("data", Encoding::Split(Arc::new(|schema_type|
                match schema_type {
                    SchemaType::Json => Encoding::Bytes,
                    SchemaType::Binary => Encoding::list(Encoding::Uint8)
                }
            )))
        ])
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug)]
pub struct GetOperationsMessage {
    get_operations: Vec<OperationHash>,
}

impl HasEncoding for GetOperationsMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("get_operations", Encoding::dynamic(Encoding::list(Encoding::Hash(HashEncoding::new(HashType::OperationHash))))),
        ])
    }
}
