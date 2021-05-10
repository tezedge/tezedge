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
use tezos_encoding::{binary_reader::{BinaryReaderError, BinaryReaderErrorKind}, encoding::{Encoding, Field, HasEncodingTest, HasEncoding}, has_encoding_test, nom::NomReader};


use crate::{non_cached_data, p2p::binary_message::BinaryMessage};
use crate::p2p::binary_message::BinaryChunk;
use crate::p2p::encoding::version::NetworkVersion;

#[derive(Serialize, Deserialize, Debug, Getters, Clone, HasEncoding, NomReader)]
pub struct ConnectionMessage {
    port: u16,
    #[get = "pub"]
    #[encoding(sized = "CRYPTO_KEY_SIZE", bytes)]
    public_key: Vec<u8>,
    #[encoding(sized = "POW_SIZE", bytes)]
    proof_of_work_stamp: Vec<u8>,
    #[encoding(sized = "NONCE_SIZE", bytes)]
    message_nonce: Vec<u8>,
    #[get = "pub"]
    version: NetworkVersion,
}

/*
impl tezos_encoding::nom::NomReader for ConnectionMessage {
    fn from_bytes(bytes: &[u8]) -> nom::IResult<&[u8], Self> {
        {
            let (bytes, port) =
                nom::number::complete::u16(nom::number::Endianness::Big)(bytes)?;
            let (bytes, public_key) = nom::combinator::flat_map(
                nom::bytes::complete::take(CRYPTO_KEY_SIZE),
                nom::combinator::map(nom::combinator::rest, Vec::from),
            )(bytes)?;
            let (bytes, proof_of_work_stamp) = nom::combinator::flat_map(
                nom::bytes::complete::take(POW_SIZE),
                nom::combinator::map(nom::combinator::rest, Vec::from),
            )(bytes)?;
            let (bytes, message_nonce) = nom::combinator::flat_map(
                nom::bytes::complete::take(NONCE_SIZE),
                nom::combinator::map(nom::combinator::rest, Vec::from),
            )(bytes)?;
            let (bytes, version) =
                <NetworkVersion as tezos_encoding::nom::NomReader>::from_bytes(bytes)?;
            Ok((
                bytes,
                ConnectionMessage {
                    port,
                    public_key,
                    proof_of_work_stamp,
                    message_nonce,
                    version,
                },
            ))
        }
    }

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

impl tezos_encoding::raw::RawReader for ConnectionMessage {
    fn from_bytes(bytes: &[u8]) -> Result<(&[u8], Self), BinaryReaderError> {
        const PUBLIC_KEY_OFF: usize = 2;
        const PROOF_OF_WORK_STAMP_OFF: usize = PUBLIC_KEY_OFF + CRYPTO_KEY_SIZE;
        const MESSAGE_NONCE_OFF: usize = PROOF_OF_WORK_STAMP_OFF + POW_SIZE;
        const NETWORK_VERSION_OFF: usize = MESSAGE_NONCE_OFF + NONCE_SIZE;
        if bytes.len() < 2 {
            return Err(BinaryReaderErrorKind::Underflow { bytes: 2 - bytes.len() }.into());
        }
        let port: u16 = (bytes[0] as u16) << 8 + bytes[1];
        if bytes.len() - PUBLIC_KEY_OFF < CRYPTO_KEY_SIZE {
            return Err(BinaryReaderErrorKind::Underflow { bytes: CRYPTO_KEY_SIZE - (bytes.len() - PUBLIC_KEY_OFF) }.into());
        }
        let public_key = bytes[PUBLIC_KEY_OFF..PROOF_OF_WORK_STAMP_OFF].to_vec();
        if bytes.len() - PROOF_OF_WORK_STAMP_OFF < POW_SIZE {
            return Err(BinaryReaderErrorKind::Underflow { bytes: POW_SIZE - (bytes.len() - PROOF_OF_WORK_STAMP_OFF) }.into());
        }
        let proof_of_work_stamp = bytes[PROOF_OF_WORK_STAMP_OFF..MESSAGE_NONCE_OFF].to_vec();
        if bytes.len() - MESSAGE_NONCE_OFF < NONCE_SIZE {
            return Err(BinaryReaderErrorKind::Underflow { bytes: NONCE_SIZE - (bytes.len() - MESSAGE_NONCE_OFF) }.into());
        }
        let message_nonce = bytes[MESSAGE_NONCE_OFF..NETWORK_VERSION_OFF].to_vec();
        let (bytes, version) = <NetworkVersion as tezos_encoding::raw::RawReader>::from_bytes(&bytes[NETWORK_VERSION_OFF..])?;
        Ok((bytes, ConnectionMessage { port, public_key, proof_of_work_stamp, message_nonce, version }))
    }
}

// TODO: Replace this by impl TryFrom with a bounded generic parameter
//       after https://github.com/rust-lang/rust/issues/50133 is resolved.
impl TryFrom<BinaryChunk> for ConnectionMessage {
    type Error = BinaryReaderError;

    fn try_from(value: BinaryChunk) -> Result<Self, Self::Error> {
        let cursor = Cursor::new(value.content());
        <ConnectionMessage as BinaryMessage>::from_bytes(cursor.into_inner())
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
            Field::new("version", NetworkVersion::encoding_test().clone()),
        ],
    )
});
