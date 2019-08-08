use std::convert::TryFrom;
use std::io::Cursor;

use serde::{Deserialize, Serialize};

use tezos_encoding::de;
use tezos_encoding::encoding::{Encoding, Field, HasEncoding};

use crate::p2p::encoding::version::Version;
use crate::p2p::message::{BinaryMessage, RawBinaryMessage};

#[derive(Serialize, Deserialize, Debug)]
pub struct ConnectionMessage {
    port: u16,
    versions: Vec<Version>,
    public_key: Vec<u8>,
    proof_of_work_stamp: Vec<u8>,
    message_nonce: Vec<u8>,
}

impl ConnectionMessage {
    pub fn new(port: u16, public_key: &str, proof_of_work_stamp: &str, message_nonce: &[u8], versions: Vec<Version>) -> Self {
        ConnectionMessage {
            port,
            versions,
            public_key: hex::decode(public_key)
                .expect("Failed to decode public ket from hex string"),
            proof_of_work_stamp: hex::decode(proof_of_work_stamp)
                .expect("Failed to decode proof of work stamp from hex string"),
            message_nonce: message_nonce.into(),
        }
    }

    pub fn get_public_key(&self) -> &Vec<u8> {
        &self.public_key
    }

    #[allow(dead_code)]
    pub fn get_nonce(&self) -> &Vec<u8> {
        &self.message_nonce
    }
}

// TODO: remove this construct
impl TryFrom<RawBinaryMessage> for ConnectionMessage {
    type Error = de::Error;

    fn try_from(value: RawBinaryMessage) -> Result<Self, Self::Error> {
        let cursor = Cursor::new(value.get_contents());
        ConnectionMessage::from_bytes(cursor.into_inner().to_vec())
    }
}

impl HasEncoding for ConnectionMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("port", Encoding::Uint16),
            Field::new("public_key", Encoding::sized(32, Encoding::Bytes)),
            Field::new("proof_of_work_stamp", Encoding::sized(24, Encoding::Bytes)),
            Field::new("message_nonce", Encoding::sized(24, Encoding::Bytes)),
            Field::new("versions", Encoding::list(Version::encoding()))
        ])
    }
}