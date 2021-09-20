// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    io::Read,
    slice::Chunks,
    sync::mpsc::{channel, TryRecvError},
    time::Duration,
};

use std::convert::{TryFrom, TryInto};

use bytes::BufMut;
use criterion::Criterion;
use crypto::{
    crypto_box::{PrecomputedKey, PublicKey, SecretKey},
    hash::{BlockHash, ContextHash, CryptoboxPublicKeyHash, OperationListListHash},
    nonce::{Nonce, NoncePair},
};
use networking::p2p::stream::{
    Crypto, EncryptedMessageReaderBase, MessageStream, CONTENT_LENGTH_MAX,
};
use slog::{debug, o, Drain};
use tezos_messages::p2p::{
    binary_message::BinaryChunk,
    encoding::{
        limits::BLOCK_HEADER_PROTOCOL_DATA_MAX_SIZE,
        peer::{PeerMessage, PeerMessageResponse},
    },
};

use tezos_messages::p2p::binary_message::BinaryRead;
use tokio::{runtime::Builder, time::Instant};

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

pub struct PeerMock<W> {
    crypt: Crypto,
    chunk_size: usize,
    write: W,
}

impl<W: std::io::Write> PeerMock<W> {
    pub fn new(precomputed_key: PrecomputedKey, nonce: Nonce, write: W) -> Self {
        Self {
            crypt: Crypto::new(precomputed_key, nonce),
            chunk_size: CONTENT_LENGTH_MAX,
            write,
        }
    }

    pub fn chunk_size(&mut self, chunk_size: usize) {
        self.chunk_size = chunk_size;
    }

    pub fn send_message<T: AsRef<[u8]>>(&mut self, message: T) {
        message
            .as_ref()
            .chunks(self.chunk_size as usize)
            .for_each(|chunk| {
                let encrypted = self.crypt.encrypt(&chunk).unwrap();
                let binary_chunk = BinaryChunk::from_content(&encrypted).unwrap();
                self.write.write_all(binary_chunk.raw()).unwrap();
            });
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
    let drain = slog_envlogger::new(drain);
    let drain = slog_async::Async::new(drain).build().fuse();

    slog::Logger::root(drain, o!())
}

pub fn read_message_bench(c: &mut Criterion, message: Vec<u8>, chunk_size: Option<usize>) {
    let root_log = new_log();

    let (addr_send, addr_recv) = channel();
    let (stop_tx, stop_rx) = channel();

    let crypto_mock = CryptoMock::new();
    let (crypto_local, crypto_remote) = (crypto_mock.local, crypto_mock.remote);
    let sender_log = root_log.new(o!());
    let receiver_log = root_log.new(o!());
    let message_len = message.len();

    let sender = std::thread::spawn(move || {
        use std::net::TcpListener;

        debug!(sender_log, "Starting listening");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let local_addr = listener.local_addr().unwrap();
        addr_send.send(local_addr).unwrap();
        let log = sender_log.new(o!("sender" => local_addr));
        debug!(log, "Listening");

        listener.set_nonblocking(true).unwrap();

        let precompute_key = crypto_remote.precompute_key;
        let nonce = crypto_remote.nonce_pair.local;

        loop {
            match listener.accept() {
                Ok((mut socket, _)) => {
                    debug!(log, "connected");
                    let mut buf = [0u8; 10];
                    socket.read_exact(&mut buf).unwrap();
                    let iters = u64::from_be_bytes(buf[2..].try_into().unwrap());
                    let mut mock = PeerMock::new(precompute_key.clone(), nonce.clone(), socket);
                    if let Some(chunk_size) = chunk_size {
                        mock.chunk_size(chunk_size);
                    }
                    for _ in 0..iters {
                        mock.send_message(message.clone());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                Err(e) => panic!("error accepting a connection: {}", e),
            }
            match stop_rx.try_recv() {
                Ok(()) => break,
                Err(TryRecvError::Empty) => (),
                Err(TryRecvError::Disconnected) => panic!("stop channel disconnected"),
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    let rt = Builder::new_multi_thread()
        .enable_io()
        .build()
        .expect("Cannot build tokio runtime");

    rt.block_on(async move {
        use tokio::net::TcpStream;

        debug!(receiver_log, "Tokio started");

        let addr = addr_recv.recv().unwrap();
        let log = receiver_log.new(o!("receiver" => addr));

        c.bench_function(
            &format!(
                "stream reading, message size: {}, chunk size: {}",
                message_len,
                chunk_size.unwrap_or(CONTENT_LENGTH_MAX)
            ),
            move |b| {
                let precompute_key = crypto_local.precompute_key.clone();
                let log = log.clone();
                let nonce = crypto_local.nonce_pair.remote.clone();
                b.iter_custom(move |iters| {
                    let log = log.clone();
                    debug!(log, "Iterating for {} times", iters);
                    let (done_tx, done_rx) = channel();
                    let precompute_key = precompute_key.clone();
                    let nonce = nonce.clone();
                    debug!(log, "Spawning tokio...");
                    tokio::spawn(async move {
                        debug!(log, "Tokio spawned...");
                        let socket = TcpStream::connect(addr).await.unwrap();
                        let stream = MessageStream::from(socket);
                        let (reader, mut writer) = stream.split();
                        debug!(log, "Sending iterations count to sender...");
                        writer
                            .write_message(
                                &BinaryChunk::from_content(&iters.to_be_bytes()).unwrap(),
                            )
                            .await
                            .unwrap();
                        let mut reader = EncryptedMessageReaderBase::new(
                            reader,
                            precompute_key,
                            nonce,
                            new_log(),
                        );
                        debug!(log, "Starting iterations");
                        let start = Instant::now();
                        for _ in 0..iters {
                            reader.read_message::<PeerMessageResponse>().await.unwrap();
                        }
                        done_tx.send(start.elapsed()).unwrap();
                        debug!(log, "Done iterating");
                    });
                    done_rx.recv().unwrap()
                })
            },
        );
        stop_tx.send(()).unwrap();
    });

    sender.join().unwrap();
}
