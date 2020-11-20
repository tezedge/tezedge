// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

#[test]
fn can_deserialize_deactivate_message() {
    let deactivate_message =
        PeerMessage::Deactivate(DeactivateMessage::new(hex::decode("8eceda2f").unwrap()));
    let resp: PeerMessageResponse = deactivate_message.into();
    let msg_bytes = resp.as_bytes().unwrap();
    let expected = hex::decode("0000000600128eceda2f").expect("Failed to decode");
    assert_eq!(expected, msg_bytes);
}

#[test]
fn deserialized_equals_serialized_message() {
    let original_message =
        PeerMessage::Deactivate(DeactivateMessage::new(hex::decode("8eceda2f").unwrap()));
    let resp: PeerMessageResponse = original_message.into();
    let msg_bytes = resp.as_bytes().unwrap();
    let deserialized = PeerMessageResponse::from_bytes(msg_bytes).expect("expected valid message");
    let deserialized_message = deserialized
        .messages()
        .get(0)
        .expect("expected message in response");
    let original_message =
        PeerMessage::Deactivate(DeactivateMessage::new(hex::decode("8eceda2f").unwrap()));

    if let (PeerMessage::Deactivate(ref orig_msg), PeerMessage::Deactivate(ref des_msg)) =
        (original_message, deserialized_message)
    {
        assert_eq!(orig_msg.deactivate(), des_msg.deactivate());
    } else {
        panic!("expected two deactivate messages")
    }
}
