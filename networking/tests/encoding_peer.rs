use failure::Error;

use networking::p2p::encoding::prelude::*;
use networking::p2p::binary_message::BinaryMessage;

#[test]
fn can_deserialize_bootstrap() -> Result<(), Error> {
    let message_bytes = hex::decode("000000020002")?;
    let messages = PeerMessageResponse::from_bytes(message_bytes).unwrap();
    assert_eq!(1, messages.get_messages().len());

    let message = messages.get_messages().get(0).unwrap();
    match message {
        PeerMessage::Bootstrap => Ok(()),
        _ => panic!("Unsupported encoding: {:?}", message)
    }
}

#[test]
fn can_serialize_bootstrap() -> Result<(), Error> {
    let message = PeerMessageResponse::from(PeerMessage::Bootstrap);
    let serialized = hex::encode(message.as_bytes()?);
    let expected = "000000020002";
    Ok(assert_eq!(expected, &serialized))
}

