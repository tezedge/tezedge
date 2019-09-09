use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};

#[derive(Serialize, Deserialize, Debug)]
pub struct AdvertiseMessage {
    pub id: Vec<String>,
}

impl HasEncoding for AdvertiseMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("id", Encoding::list(Encoding::String)),
        ])
    }
}
