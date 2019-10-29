// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Error;
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

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