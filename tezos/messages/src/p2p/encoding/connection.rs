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
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::has_encoding;

use crate::non_cached_data;
use crate::p2p::binary_message::{BinaryChunk, BinaryMessage};
use crate::p2p::encoding::version::NetworkVersion;

#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct ConnectionMessage {
    port: u16,
    #[get = "pub"]
    version: NetworkVersion,
    #[get = "pub"]
    public_key: Vec<u8>,
    proof_of_work_stamp: Vec<u8>,
    message_nonce: Vec<u8>,
}

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
has_encoding!(ConnectionMessage, CONNECTION_MESSAGE_ENCODING, {
    Encoding::Obj(vec![
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
    ])
});
