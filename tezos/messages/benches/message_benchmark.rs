use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tezos_messages::p2p::{
    binary_message::{BinaryMessage, BinaryChunk, BinaryChunkError, CONTENT_LENGTH_FIELD_BYTES},
    encoding::operation::Operation,
    binary_message::cache::CachedData,
    encoding::prelude::ConnectionMessage,
    encoding::peer::PeerMessageResponse,
    encoding::metadata::MetadataMessage,
};
use tezos_encoding::{
    binary_reader::BinaryReader,
    encoding::{Encoding, HasEncoding},
    de::from_value as deserialize_from_value,
};
use crypto::{
    crypto_box::{precompute, decrypt, CryptoError},
    nonce::{generate_nonces, NoncePair},
    hash::HashType,
};

use csv;
use serde::{Deserialize, Deserializer};
use hex::FromHex;
use std::fs::File;
use std::io::prelude::*;
use serde_json;
use bytes::Buf;

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
            let chunk = BinaryChunk::from_content(&self.0[CONTENT_LENGTH_FIELD_BYTES..chunk_length+CONTENT_LENGTH_FIELD_BYTES]);
            self.0.drain(0..chunk_length+CONTENT_LENGTH_FIELD_BYTES);
            return chunk;
        }
        return Err(BinaryChunkError::IncorrectSizeInformation{expected: chunk_length, actual: self.0.len()-CONTENT_LENGTH_FIELD_BYTES});
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

// deserialize_benchmark mimics execution of main operations in BinaryMessage::from_bytes
pub fn deserialize_benchmark(c: &mut Criterion) {
    let message_bytes = hex::decode("0090304939374e4f0f260928d4879fd5f359b4ff146f3fd37142436fb8ce1ab57af68648964efff6ca56a82b61185aec6538fa000125a2a1468416d65247660efcba15111467b9feab07dfc3dafac2d2a8c4c6dbca0b97b7239bcc4bd7ab2229b9c506022870539f6505ff56af81e5d344baa82465bae2a023afa5de27a6600e4dc85b050471ef8c3d887bb7a65700caaa98").unwrap();
    c.bench_function("operation_from_bytes", |b| b.iter(|| { Operation::from_bytes(black_box(message_bytes.clone()))}));
    c.bench_function("from_bytes_reader", |b| b.iter(|| { BinaryReader::new().read(black_box(message_bytes.clone()), &Operation::encoding()).unwrap() }));
    let value = BinaryReader::new().read(black_box(message_bytes.clone()), &Operation::encoding()).unwrap();
    c.bench_function("from_bytes_deserialize", |b| b.iter(|| deserialize_from_value::<Operation>(black_box(&value)).unwrap()));
    let mut msg: Operation = deserialize_from_value(&value).unwrap();
    if let Some(cache_writer) = msg.cache_writer() {
        c.bench_function("from_bytes_write_cache", |b| b.iter(|| cache_writer.put(black_box(&message_bytes)) ));
    }
}

// real data benchmark reads a real-life communication between two nodes and performs decrypting, deserialization and decoding
pub fn decode_stream(c: &mut Criterion) {
    let mut identity_file = File::open("benches/identity.json").unwrap();
    let mut identity_raw = String::new();
    identity_file.read_to_string(&mut identity_raw).unwrap();
    let identity:Identity = serde_json::from_str(&identity_raw).unwrap();

    let mut reader = csv::Reader::from_path("benches/tezedge_communication.csv").unwrap();
    let mut messages:Vec<Message> = vec![];
    for result in reader.deserialize() {
        let record:Message = result.unwrap();
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
    let NoncePair { mut remote, mut local } = generate_nonces(&messages[0].message, &messages[1].message, is_incoming);

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
    let mut decrypted_messages:Vec<Vec<u8>> = vec![];
    for i in 4..messages.len() {
        let decrypted_message = match messages[i].direction {
            TxRx::Sent => {
                outgoing.add_payload(&messages[i].message);
                match outgoing.drain_chunk() {
                    Ok(chunk) => {
                        match decrypt(&chunk.content(), &local, &precomputed_key) {
                            Ok(dm) => {
                                local = local.increment();
                                Ok(dm)
                            },
                            Err(e) => {
                                Err(e)
                            }
                        }
                    },
                    Err(_) => Err(CryptoError::FailedToDecrypt),
                }
            },
            TxRx::Received => {
                incoming.add_payload(&messages[i].message);
                match incoming.drain_chunk() {
                    Ok(chunk) => {
                        match decrypt(&chunk.content(), &remote, &precomputed_key) {
                            Ok(dm) => {
                                remote = remote.increment();
                                Ok(dm)
                            },
                            Err(e) => {
                                Err(e)
                            }
                        }
                    },
                    Err(_) => Err(CryptoError::FailedToDecrypt),
                }
            }
        };

        match decrypted_message {
            Ok(dm) => {
                match PeerMessageResponse::from_bytes(dm.to_owned()) {
                    Ok(_) => decrypted_messages.push(dm),
                    _ => (),
                }
            },
            _ => (),
        }
    }
    assert!(decrypted_messages.len()>0, "could not decrypt any message");
    assert!(decrypted_messages.len()==5472, "should be able do decrypt 5472 messages");
    c.bench_function("decode_stream", |b| b.iter(|| { for message in decrypted_messages.to_owned() { PeerMessageResponse::from_bytes(message);}}));
}

fn hex_to_buffer<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where D: Deserializer<'de> {
    use serde::de::Error;
    String::deserialize(deserializer)
        .and_then(|string| Vec::from_hex(&string).map_err(|err| Error::custom(err.to_string())))
}

// decode_value benchmarks measure raw performance of BinaryReader's decode_value function.
pub fn decode_value_hash_benchmark(c: &mut Criterion) {
    let mut buf =  [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31].as_ref();
    let br = BinaryReader::new();
    c.bench_function("decode_value_hash", |b| b.iter(|| { br.decode_value(&mut buf, &Encoding::Hash(HashType::PublicKeyEd25519)) }));
}

pub fn decode_value_bytes_benchmark(c: &mut Criterion) {
    let mut buf =  [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31].as_ref();
    let br = BinaryReader::new();
    c.bench_function("decode_value_bytes", |b| b.iter(|| { br.decode_value(&mut buf, &Encoding::Bytes) }));
}

pub fn decode_value_string_benchmark(c: &mut Criterion) {
    let s = String::from("This is a thirty-two byte stringThis is a thirty-two byte stringThis is a thirty-two byte stringThis is a thirty-two byte string"); // A 128-byte string.
    let mut buf =  s.as_ref();
    let br = BinaryReader::new();
    c.bench_function("decode_value_string", |b| b.iter(|| { br.decode_value(&mut buf, &Encoding::String) }));
}

criterion_group!{
    name = benches;
    config = Criterion::default();
    // targets = deserialize_benchmark, decode_stream
    targets = decode_value_hash_benchmark, decode_value_bytes_benchmark, decode_value_string_benchmark, decode_stream
}

criterion_main!(benches);
