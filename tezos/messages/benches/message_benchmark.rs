// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;
use std::path::Path;

use bytes::Buf;
use criterion::{criterion_group, criterion_main, Criterion};

use hex::FromHex;
use serde::{Deserialize, Deserializer};

use crypto::crypto_box::PrecomputedKey;
use crypto::{
    crypto_box::{CryptoKey, PublicKey},
    nonce::{generate_nonces, NoncePair},
    CryptoError,
};
use tezos_messages::p2p::{
    binary_message::{BinaryChunk, BinaryChunkError, BinaryRead, CONTENT_LENGTH_FIELD_BYTES},
    encoding::metadata::MetadataMessage,
    encoding::peer::PeerMessageResponse,
    encoding::prelude::ConnectionMessage,
};

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
        Err(BinaryChunkError::IncorrectSizeInformation {
            expected: chunk_length,
            actual: self.0.len() - CONTENT_LENGTH_FIELD_BYTES,
        })
    }

    fn add_payload(&mut self, payload: &[u8]) {
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

/// decode_stream benchmark reads a sample communication between two nodes and performs
/// decryption, deserialization and decoding.
pub fn decode_stream(c: &mut Criterion) {
    // bench data root dir
    let data_dir =
        Path::new(&env::var("CARGO_MANIFEST_DIR").expect("No `CARGO_MANIFEST_DIR` is set"))
            .join("benches");

    // read file: "benches/identity.json"
    let identity = tezos_identity::load_identity(data_dir.join("identity.json"))
        .expect("Failed to load identity");

    // read file benches/tezedge_communication.csv
    let mut reader = csv::Reader::from_path(data_dir.join("tezedge_communication.csv")).unwrap();
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
    let peer_public_key = PublicKey::from_bytes(connection_message_remote.public_key())
        .expect("PublicKey decode failed");
    let precomputed_key = PrecomputedKey::precompute(&peer_public_key, &identity.secret_key);

    let is_incoming = messages[0].direction == TxRx::Received;
    let NoncePair {
        mut remote,
        mut local,
    } = generate_nonces(&messages[0].message, &messages[1].message, is_incoming).unwrap();

    let decrypted_message = precomputed_key.decrypt(&messages[2].message[2..], &local);
    assert!(decrypted_message.is_ok(), "can't decrypt sent metadata");

    let meta_sent = MetadataMessage::from_bytes(decrypted_message.unwrap());
    assert!(meta_sent.is_ok(), "can't deserialize sent metadata");
    local = local.increment();

    let decrypted_message = precomputed_key.decrypt(&messages[3].message[2..], &remote);
    assert!(decrypted_message.is_ok(), "can't decrypt received metadata");
    let meta_received = MetadataMessage::from_bytes(decrypted_message.unwrap());
    assert!(meta_received.is_ok(), "can't deserialize received metadata");
    remote = remote.increment();

    let mut outgoing = BinaryStream::new();
    let mut incoming = BinaryStream::new();
    let mut decrypted_messages: Vec<Vec<u8>> = vec![];
    for message in messages.iter().skip(4) {
        let decrypted_message = match message.direction {
            TxRx::Sent => {
                outgoing.add_payload(&message.message);
                match outgoing.drain_chunk() {
                    Ok(chunk) => match precomputed_key.decrypt(chunk.content(), &local) {
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
                incoming.add_payload(&message.message);
                match incoming.drain_chunk() {
                    Ok(chunk) => match precomputed_key.decrypt(chunk.content(), &remote) {
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

        if let Ok(dm) = decrypted_message {
            if PeerMessageResponse::from_bytes(dm.to_owned()).is_ok() {
                decrypted_messages.push(dm)
            }
        }
    }
    assert!(
        !decrypted_messages.is_empty(),
        "could not decrypt any message"
    );
    assert_eq!(
        5472,
        decrypted_messages.len(),
        "should be able do decrypt 5472 messages"
    );
    c.bench_function("decode_stream", |b| {
        b.iter(|| {
            for message in decrypted_messages.to_owned() {
                PeerMessageResponse::from_bytes(message).unwrap();
            }
        })
    });
}

/// simulate_bootstrap_crypto benchmark: creation of PrecomputedKey, generate nonces, decrypt first message, which happens in real communication between two nodes
pub fn simulate_bootstrap_crypto(c: &mut Criterion) {
    // bench data root dir
    let data_dir =
        Path::new(&env::var("CARGO_MANIFEST_DIR").expect("No `CARGO_MANIFEST_DIR` is set"))
            .join("benches");

    // read file: "benches/identity.json"
    let identity = tezos_identity::load_identity(data_dir.join("identity.json"))
        .expect("Failed to load identity");

    // read file benches/tezedge_communication.csv
    let mut reader = csv::Reader::from_path(data_dir.join("tezedge_communication.csv")).unwrap();
    let mut messages: Vec<Message> = vec![];
    for result in reader.deserialize() {
        let record: Message = result.unwrap();
        messages.push(record);
    }

    // prepare data (we need remote connection message)
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
    let peer_public_key = PublicKey::from_bytes(connection_message_remote.public_key())
        .expect("PublicKey decode failed");

    c.bench_function("simulate_bootstrap_crypto", |b| {
        b.iter(|| {
            // create PrecomputedKey
            let precomputed_key =
                PrecomputedKey::precompute(&peer_public_key, &identity.secret_key);

            // generate nonces
            let is_incoming = messages[0].direction == TxRx::Received;
            let NoncePair { remote: _, local } =
                generate_nonces(&messages[0].message, &messages[1].message, is_incoming).unwrap();

            // decrypt message
            let decrypted_message = precomputed_key.decrypt(&messages[2].message[2..], &local);
            assert!(decrypted_message.is_ok(), "can't decrypt sent metadata");

            let meta_sent = MetadataMessage::from_bytes(decrypted_message.unwrap());
            assert!(meta_sent.is_ok(), "can't deserialize sent metadata");
        })
    });
}

fn hex_to_buffer<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    String::deserialize(deserializer)
        .and_then(|string| Vec::from_hex(&string).map_err(|err| Error::custom(err.to_string())))
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = decode_stream, simulate_bootstrap_crypto
}

criterion_main!(benches);
