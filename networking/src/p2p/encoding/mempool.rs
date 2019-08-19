use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::hash::{HashEncoding, HashType, OperationHash};

#[derive(Serialize, Deserialize, Debug)]
pub struct Mempool {
    known_valid: Vec<OperationHash>,
    pending: Vec<OperationHash>,
}

#[allow(dead_code)]
impl Mempool {
    pub fn new() -> Self {
        Mempool { pending: Vec::new(), known_valid: Vec::new() }
    }

    pub fn get_known_valid(&self) -> &Vec<OperationHash> {
        &self.known_valid
    }

    pub fn get_pending(&self) -> &Vec<OperationHash> {
        &self.pending
    }
}

impl HasEncoding for Mempool {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("known_valid", Encoding::dynamic(Encoding::list(Encoding::Hash(HashEncoding::new(HashType::OperationHash))))),
            Field::new("pending", Encoding::dynamic(Encoding::dynamic(Encoding::list(Encoding::Hash(HashEncoding::new(HashType::OperationHash)))))),
        ])
    }
}
