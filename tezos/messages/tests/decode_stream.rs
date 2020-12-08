use crypto::{
    crypto_box::{decrypt, precompute, CryptoError},
    nonce::{generate_nonces, NoncePair},
};
use tezos_messages::p2p::{
    binary_message::{BinaryChunk, BinaryChunkError, BinaryMessage, CONTENT_LENGTH_FIELD_BYTES},
    encoding::metadata::MetadataMessage,
    encoding::peer::PeerMessageResponse,
    encoding::prelude::ConnectionMessage,
};

use bytes::Buf;
use csv;
use hex::FromHex;
use serde::{Deserialize, Deserializer};
use serde_json;
use std::fs::File;
use std::io::prelude::*;

#[derive(Deserialize, Debug)]
pub struct Identity {
    pub peer_id: String,
    pub public_key: String,
    pub secret_key: String,
    pub proof_of_work_stamp: String,
}

pub struct BinaryStream(Vec<u8>);

impl BinaryStream {
    fn new() -> Self {
        BinaryStream(vec![])
    }

    fn drain_chunk(&mut self) -> Result<BinaryChunk, BinaryChunkError> {
        if self.0.len() < CONTENT_LENGTH_FIELD_BYTES {
            return Err(BinaryChunkError::MissingSizeInformation);
        }
        let chunk_length = (&self.0[0..CONTENT_LENGTH_FIELD_BYTES]).get_u16() as usize;
        if self.0.len() >= chunk_length + CONTENT_LENGTH_FIELD_BYTES {
            let chunk = BinaryChunk::from_content(
                &self.0[CONTENT_LENGTH_FIELD_BYTES..chunk_length + CONTENT_LENGTH_FIELD_BYTES],
            );
            self.0.drain(0..chunk_length + CONTENT_LENGTH_FIELD_BYTES);
            return chunk;
        }
        return Err(BinaryChunkError::IncorrectSizeInformation {
            expected: chunk_length,
            actual: self.0.len() - CONTENT_LENGTH_FIELD_BYTES,
        });
    }

    fn add_payload(&mut self, payload: &Vec<u8>) {
        self.0.extend_from_slice(payload);
    }
}

#[derive(Debug, Deserialize, PartialEq)]
enum TxRx {
    Sent,
    Received,
}

#[derive(Debug, Deserialize)]
struct Message {
    direction: TxRx,
    #[serde(deserialize_with = "hex_to_buffer")]
    message: Vec<u8>,
}

// decode_stream verifies if the file with sample communication between two nodes is correct
// for performing a benchmark of decryption, deserialization and decoding.
#[test]
pub fn decode_stream() {
    let mut identity_file = File::open("benches/identity.json").unwrap();
    let mut identity_raw = String::new();
    identity_file.read_to_string(&mut identity_raw).unwrap();
    let identity: Identity = serde_json::from_str(&identity_raw).unwrap();

    let mut reader = csv::Reader::from_path("benches/tezedge_communication.csv").unwrap();
    let mut messages: Vec<Message> = vec![];
    for result in reader.deserialize() {
        let record: Message = result.unwrap();
        messages.push(record);
    }

    let sent_data = &messages[0].message[2..];
    let recv_data = &messages[1].message[2..];
    let _connection_message_local = match ConnectionMessage::from_bytes(sent_data.to_vec()) {
        Ok(m) => m,
        Err(e) => {
            println!("Error in connection message {:?}", e);
            return;
        }
    };
    let connection_message_remote = match ConnectionMessage::from_bytes(recv_data.to_vec()) {
        Ok(m) => m,
        Err(e) => {
            println!("Error in connection message {:?}", e);
            return;
        }
    };
    let peer_public_key = connection_message_remote.public_key();
    let precomputed_key = match precompute(&hex::encode(peer_public_key), &identity.secret_key) {
        Ok(key) => key,
        Err(_) => panic!("can't read precomputed key from data stream"),
    };

    let is_incoming = messages[0].direction == TxRx::Received;
    let NoncePair {
        mut remote,
        mut local,
    } = generate_nonces(&messages[0].message, &messages[1].message, is_incoming);

    let decrypted_message = decrypt(&messages[2].message[2..], &local, &precomputed_key);
    assert!(decrypted_message.is_ok(), "can't decrypt sent metadata");
    let meta_sent = MetadataMessage::from_bytes(decrypted_message.unwrap());
    assert!(meta_sent.is_ok(), "can't deserialize sent metadata");
    local = local.increment();

    let decrypted_message = decrypt(&messages[3].message[2..], &remote, &precomputed_key);
    assert!(decrypted_message.is_ok(), "can't decrypt received metadata");
    let _meta_received = MetadataMessage::from_bytes(decrypted_message.unwrap());
    assert!(meta_sent.is_ok(), "can't deserialize received metadata");
    remote = remote.increment();

    let mut outgoing = BinaryStream::new();
    let mut incoming = BinaryStream::new();
    let mut decrypted_messages: Vec<Vec<u8>> = vec![];
    for i in 4..messages.len() {
        let decrypted_message = match messages[i].direction {
            TxRx::Sent => {
                outgoing.add_payload(&messages[i].message);
                match outgoing.drain_chunk() {
                    Ok(chunk) => match decrypt(&chunk.content(), &local, &precomputed_key) {
                        Ok(dm) => {
                            local = local.increment();
                            Ok(dm)
                        }
                        Err(e) => Err(e),
                    },
                    Err(_) => Err(CryptoError::FailedToDecrypt),
                }
            }
            TxRx::Received => {
                incoming.add_payload(&messages[i].message);
                match incoming.drain_chunk() {
                    Ok(chunk) => match decrypt(&chunk.content(), &remote, &precomputed_key) {
                        Ok(dm) => {
                            remote = remote.increment();
                            Ok(dm)
                        }
                        Err(e) => Err(e),
                    },
                    Err(_) => Err(CryptoError::FailedToDecrypt),
                }
            }
        };

        match decrypted_message {
            Ok(dm) => match PeerMessageResponse::from_bytes(dm.to_owned()) {
                Ok(_) => decrypted_messages.push(dm),
                _ => (),
            },
            _ => (),
        }
    }
    assert!(
        decrypted_messages.len() > 0,
        "could not decrypt any message"
    );
    assert!(
        decrypted_messages.len() == 5472,
        "should be able do decrypt 5472 messages, only done {}",
        decrypted_messages.len()
    );
}

fn hex_to_buffer<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    String::deserialize(deserializer)
        .and_then(|string| Vec::from_hex(&string).map_err(|err| Error::custom(err.to_string())))
}
