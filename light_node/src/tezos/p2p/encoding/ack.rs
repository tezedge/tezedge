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

#[cfg(test)]
mod tests {
    use failure::Error;

    use crate::tezos::p2p::message::BinaryMessage;

    use super::*;

    #[test]
    fn can_serialize_ack() -> Result<(), Error> {
        let message = AckMessage::Ack;
        let serialized = hex::encode(message.as_bytes()?);
        let expected = "00";
        Ok(assert_eq!(expected, &serialized))
    }

    #[test]
    fn can_deserialize_ack() -> Result<(), Error> {
        let message_bytes = hex::decode("00")?;
        let message = AckMessage::from_bytes(message_bytes)?;
        Ok(assert_eq!(AckMessage::Ack, message))
    }

    #[test]
    fn can_serialize_nack() -> Result<(), Error> {
        let message = AckMessage::Nack;
        let serialized = hex::encode(message.as_bytes()?);
        let expected = "ff";
        Ok(assert_eq!(expected, &serialized))
    }

    #[test]
    fn can_deserialize_nack() -> Result<(), Error> {
        let message_bytes = hex::decode("ff")?;
        let message = AckMessage::from_bytes(message_bytes)?;
        Ok(assert_eq!(AckMessage::Nack, message))
    }
}