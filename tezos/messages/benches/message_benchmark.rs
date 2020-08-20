use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tezos_messages::p2p::{
    binary_message::BinaryMessage,
    encoding::operation::Operation,
    binary_message::cache::CachedData,
    encoding::prelude::ConnectionMessage
};
use tezos_encoding::{binary_reader::BinaryReader, encoding::HasEncoding};
use tezos_encoding::de::from_value as deserialize_from_value;
use crypto::{
    crypto_box::{precompute, decrypt},
    hash::HashType,
    nonce::{generate_nonces, NoncePair},
};

use csv;
use serde::{Deserialize, Deserializer};
use hex::FromHex;
use std::fs::File;
use std::io::prelude::*;
use serde_json;

#[derive(Deserialize, Debug)]
pub struct Identity {
    pub peer_id: String,
    pub public_key: String,
    pub secret_key: String,
    pub proof_of_work_stamp: String,
}

#[derive(Debug, Deserialize, PartialEq)]
enum TxRx {
    Sent,
    Received,
}

#[derive(Debug, Deserialize)]
struct Message {
    Direction: TxRx,
    #[serde(deserialize_with = "hex_to_buffer")]
    Message: Vec<u8>,
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
pub fn real_data_benchmark(c: &mut Criterion) {
    let mut identity_file = File::open("benches/identity.json").unwrap();
    let mut identity_raw = String::new();
    identity_file.read_to_string(&mut identity_raw).unwrap();
    let identity:Identity = serde_json::from_str(&identity_raw).unwrap();
    println!("{}",identity.secret_key);

    let mut reader = csv::Reader::from_path("benches/tezedge_communication.csv").unwrap();
    let mut messages:Vec<Message> = vec![];
    for result in reader.deserialize() {
        let record:Message = result.unwrap();
        messages.push(record);
    }
    println!("{}",messages.len());

    let recv_data = &messages[0].Message;
    let sent_data = &messages[1].Message;
    let connection_message = ConnectionMessage::from_bytes(recv_data.to_vec()).unwrap();
    let peer_public_key = connection_message.public_key();
    let precomputed_key = match precompute(&hex::encode(peer_public_key), &identity.secret_key) {
        Ok(key) => key,
        Err(_) => panic!("can't read precomputed key from data stream"),
    };

    let is_incoming = messages[4].Direction == TxRx::Received;
    let NoncePair { remote, local } = generate_nonces(sent_data, recv_data, is_incoming);
    println!("noncences: {:?};{:?}", remote, local);

    let decrypted_message = decrypt(&messages[4].Message, &                                                               local, &precomputed_key);
}

fn hex_to_buffer<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where D: Deserializer<'de> {
    use serde::de::Error;
    String::deserialize(deserializer)
        .and_then(|string| Vec::from_hex(&string).map_err(|err| Error::custom(err.to_string())))
}


criterion_group!{
    name = benches;
    config = Criterion::default();
    // targets = deserialize_benchmark, real_data_benchmark
    targets = real_data_benchmark
}

criterion_main!(benches);
