use failure::Error;

use networking::p2p::encoding::prelude::*;
use networking::p2p::message::BinaryMessage;

#[test]
fn can_deserialize_protocol() -> Result<(), Error> {
    let message_bytes = hex::decode(include_str!("resources/encoding_protocol.bytes"))?;
    let message = Protocol::from_bytes(message_bytes)?;
    assert_eq!(68, message.components().len());
    Ok(assert_eq!(0, message.expected_env_version()))
}