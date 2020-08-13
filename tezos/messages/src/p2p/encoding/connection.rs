// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;
use std::io::Cursor;

use getset::Getters;
use serde::{Deserialize, Serialize};

use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_encoding::encoding::{Encoding, Field, FieldName, HasEncoding};

use crate::p2p::binary_message::{BinaryChunk, BinaryMessage};
use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};
use crate::p2p::encoding::version::NetworkVersion;

#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct ConnectionMessage {
    pub port: u16,
    #[get = "pub"]
    pub versions: Vec<NetworkVersion>,
    #[get = "pub"]
    pub public_key: Vec<u8>,
    pub proof_of_work_stamp: Vec<u8>,
    pub message_nonce: Vec<u8>,
    #[serde(skip_serializing)]
    body: BinaryDataCache
}

impl ConnectionMessage {
    pub fn new(port: u16, public_key: &str, proof_of_work_stamp: &str, message_nonce: &[u8], versions: Vec<NetworkVersion>) -> Self {
        ConnectionMessage {
            port,
            versions,
            public_key: hex::decode(public_key)
                .expect("Failed to decode public ket from hex string"),
            proof_of_work_stamp: hex::decode(proof_of_work_stamp)
                .expect("Failed to decode proof of work stamp from hex string"),
            message_nonce: message_nonce.into(),
            body: Default::default(),
        }
    }
}

// TODO: Replace this by impl TryFrom with a bounded generic parameter
//       after https://github.com/rust-lang/rust/issues/50133 is resolved.
impl TryFrom<BinaryChunk> for ConnectionMessage {
    type Error = BinaryReaderError;

    fn try_from(value: BinaryChunk) -> Result<Self, Self::Error> {
        let cursor = Cursor::new(value.content());
        ConnectionMessage::from_bytes(cursor.into_inner().to_vec())
    }
}

impl HasEncoding for ConnectionMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new(FieldName::Port, Encoding::Uint16),
            Field::new(FieldName::PublicKey, Encoding::sized(32, Encoding::Bytes)),
            Field::new(FieldName::ProofOfWorkStamp, Encoding::sized(24, Encoding::Bytes)),
            Field::new(FieldName::MessageNonce, Encoding::sized(24, Encoding::Bytes)),
            Field::new(FieldName::Versions, Encoding::list(NetworkVersion::encoding()))
        ])
    }
}

impl CachedData for ConnectionMessage {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}