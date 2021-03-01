// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Error;
use tezos_encoding::binary_reader::BinaryReaderErrorKind;
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

// TODO update to eof
#[test]
#[ignore]
fn can_t_deserialize_empty_message() -> Result<(), Error> {
    // dynamic block of size 0
    let bytes = hex::decode("00000000")?;
    let err = PeerMessageResponse::from_bytes(bytes).expect_err("Error is expected");
    println!("{:?}", err);
    assert!(matches!(
        err.kind(),
        BinaryReaderErrorKind::Underflow { .. }
    ));
    Ok(())
}

#[test]
fn can_t_deserialize_message_list() -> Result<(), Error> {
    // dynamic block of size 2, bootstrap + disconnect
    let bytes = hex::decode("0000000400020001")?;
    let err = PeerMessageResponse::from_bytes(bytes).expect_err("Error is expected");
    println!("{:?}", err);
    assert!(matches!(err.kind(), BinaryReaderErrorKind::Overflow { .. }));
    Ok(())
}
