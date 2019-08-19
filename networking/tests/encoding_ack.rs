use failure::Error;

use networking::p2p::encoding::prelude::*;
use networking::p2p::message::BinaryMessage;

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