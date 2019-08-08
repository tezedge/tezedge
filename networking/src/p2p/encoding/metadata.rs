use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};

#[derive(Serialize, Deserialize, Debug)]
pub struct MetadataMessage {
    pub disable_mempool: bool,
    pub private_node: bool,
}

impl MetadataMessage {
    pub fn new(disable_mempool: bool, private_node: bool) -> Self {
        MetadataMessage { disable_mempool, private_node }
    }
}

impl HasEncoding for MetadataMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("disable_mempool", Encoding::Bool),
            Field::new("private_node", Encoding::Bool)
        ])
    }
}
