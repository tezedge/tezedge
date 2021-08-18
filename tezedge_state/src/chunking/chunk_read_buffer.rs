// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::io::{self, Read};

use bytes::Buf;
use tezos_messages::p2p::binary_message::{BinaryChunk, CONTENT_LENGTH_FIELD_BYTES};

/// Read buffer handshake messages.
///
/// Handshake messages(connection, metadata, ack) can only occupy one
/// chunk, but PeerMessage can be sent using multiple chunks.
///
/// PeerMessage can be big in size, hence can be sent using multiple chunks.
pub type HandshakeReadBuffer = ChunkReadBuffer;

pub trait BinaryMessageContent {
    fn binary_message_content(&self) -> &[u8];
}

pub struct BinaryChunkRef<'a>(&'a [u8]);

impl<'a> BinaryChunkRef<'a> {
    #[inline]
    pub fn raw(&self) -> &[u8] {
        self.0
    }

    #[inline]
    pub fn content(&self) -> &[u8] {
        &self.0[CONTENT_LENGTH_FIELD_BYTES..]
    }
}

impl<'a> BinaryMessageContent for BinaryChunkRef<'a> {
    #[inline]
    fn binary_message_content(&self) -> &[u8] {
        self.content()
    }
}

impl BinaryMessageContent for BinaryChunk {
    #[inline]
    fn binary_message_content(&self) -> &[u8] {
        self.content()
    }
}

/// Chunk read buffer.
///
/// Reads first 2 bytes corresponding to the size of the chunk and reads
/// that amount of bytes until it's done.
///
/// When it's [ChunkReadBuffer::is_finished], to get the result we need to call:
/// - [ChunkReadBuffer::take_if_ready] to take owned [BinaryChunk].
/// - [ChunkReadBuffer::take_ref_if_ready] to take [BinaryChunkRef].
///   This way we can reuse `ChunkReadBuffer` for multiple chunks
///   and avoid extra allocations, since buffer won't be moved/deallocated,
///   when taking the result, unlike `take_if_ready` method.
#[derive(Debug, Clone)]
pub struct ChunkReadBuffer {
    buf: Vec<u8>,
    expected_len: usize,
    index: usize,
}

impl ChunkReadBuffer {
    pub fn new() -> Self {
        Self {
            buf: vec![],
            expected_len: 0,
            index: 0,
        }
    }

    /// Check if reading chunk is finished.
    pub fn is_finished(&self) -> bool {
        self.index >= self.expected_len.max(CONTENT_LENGTH_FIELD_BYTES)
    }

    /// Next slice of buffer that we can write to.
    fn next_slice(&mut self) -> &mut [u8] {
        let len = self.expected_len.max(CONTENT_LENGTH_FIELD_BYTES);

        if self.buf.len() < len {
            self.buf.resize(len, 0);
        }

        &mut self.buf[self.index..len]
    }

    /// Returns if more might be available to be read.
    pub fn read_from<R: Read>(&mut self, reader: &mut R) -> Result<(), io::Error> {
        loop {
            let size = reader.read(self.next_slice())?;

            self.index += size;

            if self.expected_len == 0 && self.index >= CONTENT_LENGTH_FIELD_BYTES {
                self.expected_len = CONTENT_LENGTH_FIELD_BYTES + (&self.buf[..]).get_u16() as usize;
            }
            if self.is_finished() {
                return Ok(());
            } else if size == 0 {
                return Err(io::Error::new(io::ErrorKind::WouldBlock, "read 0 bytes"));
            }
        }
    }

    /// Take a reference for buffer and allow it's reuse for reading
    /// further chunks to avoid extra allocations.
    pub fn take_ref_if_ready<'a>(&'a mut self) -> Option<BinaryChunkRef<'a>> {
        if !self.is_finished() {
            return None;
        }
        let chunk = BinaryChunkRef(&self.buf[..self.expected_len]);
        self.index = 0;
        self.expected_len = 0;

        Some(chunk)
    }

    /// Consume buffer and replace it with new empty one.
    pub fn take_if_ready(&mut self) -> Option<BinaryChunk> {
        if !self.is_finished() {
            return None;
        }
        // TODO: use reference instead
        let mut buf = std::mem::take(&mut self.buf);
        buf.truncate(self.expected_len);
        self.clear();

        Some(BinaryChunk::from_raw(buf).unwrap())
    }

    pub fn clear(&mut self) {
        self.index = 0;
        self.expected_len = 0;
    }
}
