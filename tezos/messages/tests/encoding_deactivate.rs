// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

#[test]
fn can_deserialize_deactivate_message() {
    let deactivate_message = PeerMessage::Deactivate(DeactivateMessage::new(hex::decode("8eceda2f").unwrap()));
    let resp: PeerMessageResponse = deactivate_message.into();
    let msg_bytes = resp.as_bytes().unwrap();
    let expected = hex::decode("0000000600128eceda2f").expect("Failed to decode");
    assert_eq!(expected, msg_bytes);
}