// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;
use std::io::Cursor;

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::crypto_box::{PublicKey, CRYPTO_KEY_SIZE};
use crypto::nonce::{Nonce, NONCE_SIZE};
use crypto::proof_of_work::{ProofOfWork, POW_SIZE};
use crypto::CryptoError;
use tezos_encoding::{binary_reader::BinaryReaderError, encoding::{Encoding, Field, HasEncoding, HasEncodingTest}, has_encoding_test};


use crate::non_cached_data;
use crate::p2p::binary_message::{BinaryChunk, BinaryMessage};
use crate::p2p::encoding::version::NetworkVersion;

use tezos_encoding::encoding::HasEncodingOld;
use tezos_encoding::encoding::HasEncodingDerived;
use tezos_encoding::nom::NomReader;

#[derive(Serialize, Deserialize, Debug, Getters, Clone, tezos_encoding_derive::HasEncoding)]
pub struct ConnectionMessage {
    port: u16,
    #[get = "pub"]
    #[encoding(Sized("CRYPTO_KEY_SIZE", Bytes))]
    public_key: Vec<u8>,
    #[encoding(Sized("POW_SIZE", Bytes))]
    proof_of_work_stamp: Vec<u8>,
    #[encoding(Sized("NONCE_SIZE", Bytes))]
    message_nonce: Vec<u8>,
    #[get = "pub"]
    version: NetworkVersion,
}

impl tezos_encoding::nom::NomReader for ConnectionMessage {
    fn from_bytes_nom(bytes: &[u8]) -> nom::IResult<&[u8], Self> {
        let (bytes, port) = nom::number::complete::u16(nom::number::Endianness::Big)(bytes)?;
        let (bytes, public_key) = nom::combinator::map_parser(
            nom::bytes::complete::take(CRYPTO_KEY_SIZE),
            nom::combinator::map(nom::combinator::rest, Vec::from)
        )(bytes)?;
        let (bytes, proof_of_work_stamp) = nom::combinator::map_parser(
            nom::bytes::complete::take(POW_SIZE),
            nom::combinator::map(nom::combinator::rest, Vec::from)
        )(bytes)?;
        let (bytes, message_nonce) = nom::combinator::map_parser(
            nom::bytes::complete::take(NONCE_SIZE),
            nom::combinator::map(nom::combinator::rest, Vec::from)
        )(bytes)?;
        let (bytes, version) = NetworkVersion::from_bytes_nom(bytes)?;
        Ok((bytes, ConnectionMessage { port, public_key, proof_of_work_stamp, message_nonce, version }))
    }
}

/*
 * let (bytes, port) = nom::u16()?;
 * let (bytes, public_key) = num::
*/

impl ConnectionMessage {
    pub fn try_new(
        port: u16,
        public_key: &PublicKey,
        proof_of_work_stamp: &ProofOfWork,
        message_nonce: Nonce,
        version: NetworkVersion,
    ) -> Result<Self, CryptoError> {
        Ok(ConnectionMessage {
            port,
            version,
            public_key: public_key.as_ref().as_ref().to_vec(),
            proof_of_work_stamp: proof_of_work_stamp.as_ref().to_vec(),
            message_nonce: message_nonce.get_bytes()?.into(),
        })
    }
}

// TODO: Replace this by impl TryFrom with a bounded generic parameter
//       after https://github.com/rust-lang/rust/issues/50133 is resolved.
impl TryFrom<BinaryChunk> for ConnectionMessage {
    type Error = BinaryReaderError;

    fn try_from(value: BinaryChunk) -> Result<Self, Self::Error> {
        let cursor = Cursor::new(value.content());
        ConnectionMessage::from_bytes(cursor.into_inner())
    }
}

non_cached_data!(ConnectionMessage);
has_encoding_test!(ConnectionMessage, CONNECTION_MESSAGE_ENCODING, {
    Encoding::Obj(
        "ConnectionMessage",
        vec![
            Field::new("port", Encoding::Uint16),
            Field::new(
                "public_key",
                Encoding::sized(CRYPTO_KEY_SIZE, Encoding::Bytes),
            ),
            Field::new(
                "proof_of_work_stamp",
                Encoding::sized(POW_SIZE, Encoding::Bytes),
            ),
            Field::new(
                "message_nonce",
                Encoding::sized(NONCE_SIZE, Encoding::Bytes),
            ),
            Field::new("version", NetworkVersion::encoding().clone()),
        ],
    )
});
