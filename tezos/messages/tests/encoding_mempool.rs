// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Error;
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

#[test]
fn can_serialize_mempool() -> Result<(), Error> {
    let message = Mempool::default();
    let serialized = hex::encode(message.as_bytes()?);
    let expected = "000000000000000400000000";
    Ok(assert_eq!(expected, &serialized))
}
