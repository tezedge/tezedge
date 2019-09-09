use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::hash::{HashEncoding, HashType, OperationHash};

#[derive(Serialize, Deserialize, Debug)]
pub struct Mempool {
    pub known_valid: Vec<OperationHash>,
    pub pending: Vec<OperationHash>,
}

impl HasEncoding for Mempool {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("known_valid", Encoding::dynamic(Encoding::list(Encoding::Hash(HashEncoding::new(HashType::OperationHash))))),
            Field::new("pending", Encoding::dynamic(Encoding::dynamic(Encoding::list(Encoding::Hash(HashEncoding::new(HashType::OperationHash)))))),
        ])
    }
}
