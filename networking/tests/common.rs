// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::slice::Chunks;

use std::convert::TryFrom;

use bytes::BufMut;
use crypto::{
    crypto_box::{PrecomputedKey, PublicKey, SecretKey},
    hash::{BlockHash, ContextHash, CryptoboxPublicKeyHash, OperationListListHash},
    nonce::{Nonce, NoncePair},
};
use networking::p2p::stream::{Crypto, CONTENT_LENGTH_MAX};
use slog::{o, Drain};
use tezos_messages::p2p::{
    binary_message::BinaryChunk,
    encoding::{
        limits::BLOCK_HEADER_PROTOCOL_DATA_MAX_SIZE,
        peer::{PeerMessage, PeerMessageResponse},
    },
};
use tokio_test::io::{Builder, Mock};

use tezos_messages::p2p::binary_message::BinaryRead;

pub struct BinaryChunks<'a> {
    chunks: Chunks<'a, u8>,
}

impl<'a> From<Chunks<'a, u8>> for BinaryChunks<'a> {
    fn from(chunks: Chunks<'a, u8>) -> Self {
        Self { chunks }
    }
}

impl Iterator for BinaryChunks<'_> {
    type Item = BinaryChunk;
    fn next(&mut self) -> Option<Self::Item> {
        self.chunks
            .next()
            .map(|chunk| BinaryChunk::from_content(chunk).expect("Error constructing binary chunk"))
    }
}

static REMOTE_MSG: [u8; 32] = [0x0f; 32];
static LOCAL_MSG: [u8; 32] = [0xf0; 32];

pub struct CryptoMock {
    pub local: CryptoMockHalf,
    pub remote: CryptoMockHalf,
}

impl CryptoMock {
    pub fn new() -> Self {
        let local_nonce_pair =
            crypto::nonce::generate_nonces(&LOCAL_MSG, &REMOTE_MSG, false).unwrap();
        let remote_nonce_pair =
            crypto::nonce::generate_nonces(&REMOTE_MSG, &LOCAL_MSG, true).unwrap();

        let (local_sk, local_pk, local_pkh) = crypto::crypto_box::random_keypair().unwrap();
        let (remote_sk, remote_pk, remote_pkh) = crypto::crypto_box::random_keypair().unwrap();

        let local_precomp_key = PrecomputedKey::precompute(&remote_pk, &local_sk);
        let remote_precomp_key = PrecomputedKey::precompute(&local_pk, &remote_sk);

        CryptoMock {
            local: CryptoMockHalf {
                sk: local_sk,
                pk: local_pk,
                pkh: local_pkh,
                precompute_key: local_precomp_key,
                nonce_pair: local_nonce_pair,
            },
            remote: CryptoMockHalf {
                sk: remote_sk,
                pk: remote_pk,
                pkh: remote_pkh,
                precompute_key: remote_precomp_key,
                nonce_pair: remote_nonce_pair,
            },
        }
    }
}

pub struct CryptoMockHalf {
    pub sk: SecretKey,
    pub pk: PublicKey,
    pub pkh: CryptoboxPublicKeyHash,
    pub precompute_key: PrecomputedKey,
    pub nonce_pair: NoncePair,
}

pub struct PeerMock {
    crypt: Crypto,
    chunk_size: usize,
    builder: Builder,
}

impl PeerMock {
    pub fn new(precomputed_key: PrecomputedKey, nonce: Nonce) -> Self {
        Self {
            crypt: Crypto::new(precomputed_key, nonce),
            chunk_size: CONTENT_LENGTH_MAX,
            builder: Builder::new(),
        }
    }

    pub fn chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    pub fn incoming_message<T: AsRef<[u8]>>(&mut self, encoded_message: T) {
        encoded_message
            .as_ref()
            .chunks(self.chunk_size as usize)
            .for_each(|chunk| {
                let encrypted = self.crypt.encrypt(&chunk).unwrap();
                let binary_chunk = BinaryChunk::from_content(&encrypted).unwrap();
                self.builder.read(binary_chunk.raw());
            });
    }

    pub fn outgoing_message<T: AsRef<[u8]>>(&mut self, encoded_message: T) {
        encoded_message
            .as_ref()
            .chunks(self.chunk_size as usize)
            .for_each(|chunk| {
                let encrypted = self.crypt.encrypt(&chunk).unwrap();
                let binary_chunk = BinaryChunk::from_content(&encrypted).unwrap();
                self.builder.write(binary_chunk.raw());
            });
    }

    pub fn get_mock(&mut self) -> Mock {
        self.builder.build()
    }
}

pub fn data_sample(size: u16, byte: u8) -> Vec<u8> {
    std::iter::repeat(byte).take(size.into()).collect()
}

pub fn block_header_message_encoded(data_size: usize) -> Vec<u8> {
    let mut res = vec![];
    res.put_u16(0x0021); // Tag

    res.put_u32(28014); // level
    res.put_u8(0x01); // proto
    res.put_slice(
        BlockHash::try_from("BKjYUUtYXtXjEuL49jB8ZbFwVdg4hU6U7oKKSC5vp6stYsfFDVN")
            .unwrap()
            .as_ref(),
    ); // predecessor
    res.put_u64(1544713848); // timestamp
    res.put_u8(0x04); // validation pass
    res.put_slice(
        OperationListListHash::try_from("LLoZi3xywrX9swZQgC82m7vj5hmuz6LGAatNq2Muh34oNn71JruZs")
            .unwrap()
            .as_ref(),
    ); // operation hash
    res.put_u32(0x00000000); // empty fitness
    res.put_slice(
        ContextHash::try_from("CoWZVRSM6DdNUpn3mamy7e8rUSxQVWkQCQfJBg7DrTVXUjzGZGCa")
            .unwrap()
            .as_ref(),
    ); // context

    let data_size = std::cmp::min(data_size, BLOCK_HEADER_PROTOCOL_DATA_MAX_SIZE);
    res.extend(std::iter::repeat(0xff).take(data_size));

    let mut res1: Vec<u8> = vec![];
    res1.put_u32(res.len() as u32);
    res1.extend(res);

    assert!(matches!(
        PeerMessageResponse::from_bytes(res1.clone())
            .unwrap()
            .message(),
        PeerMessage::BlockHeader(_)
    ));

    res1
}

pub fn new_log() -> slog::Logger {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain)
        .build()
        .filter_level(slog::Level::Debug)
        .fuse();

    slog::Logger::root(drain, o!())
}
