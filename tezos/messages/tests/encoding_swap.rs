use crypto::hash::HashType;
use failure::Error;
use std::{convert::TryInto, iter};
use tezos_encoding::binary_reader::{ActualSize, BinaryReaderErrorKind};
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::limits::*;
use tezos_messages::p2p::encoding::swap::*;

#[test]
fn can_serialize_swap_max() -> Result<(), Error> {
    let point = iter::repeat('x')
        .take(P2P_POINT_MAX_LENGTH)
        .collect::<String>();
    let peer_id = [0; HashType::CryptoboxPublicKeyHash.size()]
        .as_ref()
        .try_into()?;
    let message = SwapMessage::new(point, peer_id);
    let res = message.as_bytes();
    assert!(res.is_ok());
    println!("{}", hex::encode(res.unwrap()));
    Ok(())
}

#[test]
fn can_t_serialize_swap_point_max_plus() -> Result<(), Error> {
    let point = iter::repeat('x')
        .take(P2P_POINT_MAX_LENGTH + 1)
        .collect::<String>();
    let peer_id = [0; HashType::CryptoboxPublicKeyHash.size()]
        .as_ref()
        .try_into()?;
    let message = SwapMessage::new(point, peer_id);
    let res = message.as_bytes();
    assert!(res.is_err());
    Ok(())
}

#[test]
fn can_deserialize_swap_max() -> Result<(), Error> {
    let encoded = hex::decode(data::SWAP_MESSAGE_MAX)?;
    let message = SwapMessage::from_bytes(encoded)?;
    assert_eq!(message.point().len(), P2P_POINT_MAX_LENGTH);
    Ok(())
}

#[test]
fn can_t_deserialize_swap_point_max_plus() -> Result<(), Error> {
    let encoded = hex::decode(data::SWAP_MESSAGE_POINT_OVER_MAX)?;
    let err = SwapMessage::from_bytes(encoded).expect_err("Error is expected");
    assert!(matches!(
        err.kind(),
        BinaryReaderErrorKind::EncodingBoundaryExceeded {
            name: _,
            boundary: P2P_POINT_MAX_SIZE,
            actual: ActualSize::Exact(actual),
        } if actual == P2P_POINT_MAX_SIZE + 1
    ));
    Ok(())
}

#[test]
fn can_t_deserialize_swap_peer_id_max_plus() -> Result<(), Error> {
    let encoded = hex::decode(data::SWAP_MESSAGE_PEER_ID_OVER_MAX)?;
    let err = SwapMessage::from_bytes(encoded).expect_err("Error is expected");
    assert!(matches!(
        err.kind(),
        BinaryReaderErrorKind::Overflow { bytes: 1 }
    ));
    Ok(())
}

mod data {
    pub const SWAP_MESSAGE_MAX: &str =              "0000002f787878787878787878787878787878787878787878787878787878787878787878787878787878787878787878787879797979797979797979797979797979";
    pub const SWAP_MESSAGE_POINT_OVER_MAX: &str =   "0000003078787878787878787878787878787878787878787878787878787878787878787878787878787878787878787878787879797979797979797979797979797979";
    pub const SWAP_MESSAGE_PEER_ID_OVER_MAX: &str = "0000002f78787878787878787878787878787878787878787878787878787878787878787878787878787878787878787878787979797979797979797979797979797979";
}
