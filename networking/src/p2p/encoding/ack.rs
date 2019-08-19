use std::mem::size_of;

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, HasEncoding, Tag, TagMap};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum AckMessage {
    Ack,
    Nack,
}

impl HasEncoding for AckMessage {
    fn encoding() -> Encoding {
        Encoding::Tags(
            size_of::<u8>(),
            TagMap::new(&[
                Tag::new(0x00, "Ack", Encoding::Unit),
                Tag::new(0xFF, "Nack", Encoding::Unit),
            ])
        )
    }
}
