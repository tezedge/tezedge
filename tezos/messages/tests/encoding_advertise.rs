// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Error;
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

#[test]
fn can_deserialize_advertise() -> Result<(), Error> {
    let message_bytes = hex::decode("0000001e5b666538303a3a653832383a323039643a3230653a633061655d3a333735000000133233342e3132332e3132342e39313a39383736000000133132332e3132332e3132342e32313a39383736")?;
    let message = AdvertiseMessage::from_bytes(message_bytes)?;
    assert_eq!(3, message.id().len());
    assert_eq!("[fe80::e828:209d:20e:c0ae]:375", &message.id()[0]);
    assert_eq!("234.123.124.91:9876", &message.id()[1]);
    Ok(assert_eq!("123.123.124.21:9876", &message.id()[2]))
}