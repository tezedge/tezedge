use tezos_messages::p2p::{
    binary_message::BinaryMessage,
    // encoding::operation::Operation,
    // binary_message::cache::CachedData,
    encoding::prelude::ConnectionMessage,
    encoding::peer::PeerMessageResponse,
    encoding::metadata::MetadataMessage,
};
// use tezos_encoding::{binary_reader::BinaryReader, encoding::HasEncoding};
// use tezos_encoding::de::from_value as deserialize_from_value;
use crypto::{
    crypto_box::{precompute, decrypt},
    // hash::HashType,
    nonce::{generate_nonces, NoncePair, Nonce},
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
    direction: TxRx,
    #[serde(deserialize_with = "hex_to_buffer")]
    message: Vec<u8>,
}

// real data benchmark reads a real-life communication between two nodes and performs decrypting, deserialization and decoding
#[test]
pub fn decode_stream() {
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
    let connection_message_local = match ConnectionMessage::from_bytes(sent_data.to_vec()) {
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
    println!("Received connection message: {:?}", connection_message_remote);
    let peer_public_key = connection_message_remote.public_key();
    let precomputed_key = match precompute(&hex::encode(peer_public_key), &identity.secret_key) {
        Ok(key) => key,
        Err(_) => panic!("can't read precomputed key from data stream"),
    };

    let is_incoming = messages[0].direction == TxRx::Received;
    let NoncePair { remote, local } = generate_nonces(&messages[0].message, &messages[1].message, is_incoming);
    println!("noncences: {:?};{:?}", remote, local);

    let decrypted_message = decrypt(&messages[2].message[2..], &local, &precomputed_key).unwrap();
    let meta_sent = MetadataMessage::from_bytes(decrypted_message);
    println!("Metadata sent: {:?}", meta_sent);
    local.increment();

    let decrypted_message = decrypt(&messages[3].message[2..], &remote, &precomputed_key).unwrap();
    let meta_received = MetadataMessage::from_bytes(decrypted_message);
    println!("Metadata received: {:?}", meta_received);
    remote.increment();

    let mut outgoing:Vec<u8> = vec![];
    let mut incoming:Vec<u8> = vec![];
    for i in 4..100 {
        let decrypted_message = match messages[i].direction {
            TxRx::Sent => {
                outgoing.extend_from_slice(&messages[i].message[2..]);
                match decrypt(&outgoing, &local, &precomputed_key) {
                    Ok(dm) => {
                        local.increment();
                        Ok(dm)
                    },
                    Err(e) => {
                        Err(e)
                    }
                }
            },
            TxRx::Received => {
                incoming.extend_from_slice(&messages[i].message[2..]);
                match decrypt(&incoming, &remote, &precomputed_key) {
                    Ok(dm) => {
                        remote.increment();
                        Ok(dm)
                    },
                    Err(e) => {
                        Err(e)
                    }
                }
            }
        };

        match decrypted_message {
            Ok(dm) => { println!("{}: {:?}", i, dm) },
            _ => ()
        }
    }
    // let decrypted_message = decrypt(&messages[8].message[2..], &local, &precomputed_key);
    // println!("Decrypted message7: {:?}", decrypted_message);
    // // let pm_response = PeerMessageResponse::from_bytes(decrypted_message);
    // // println!("Deserialized message4: {:?}", pm_response);
    // let decrypted_message = decrypt(&messages[5].message[2..], &remote, &precomputed_key);
    // println!("Decrypted message5: {:?}", decrypted_message);
}

fn hex_to_buffer<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where D: Deserializer<'de> {
    use serde::de::Error;
    String::deserialize(deserializer)
        .and_then(|string| Vec::from_hex(&string).map_err(|err| Error::custom(err.to_string())))
}

