// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::io::{self, Read};

use tezos_messages::p2p::{
    binary_message::{BinaryRead, SizeFromChunk},
    encoding::peer::PeerMessageResponse,
};

use super::{ChunkReadBuffer, ReadMessageError};
use crate::PeerCrypto;

/// Read buffer for connected peer(`PeerMessage`).
///
/// This message can be split into multiple chunks. Encrypted part of
/// the first chunk includes information (`4 bytes`) about whole message's
/// size. We need to process chunks until decrypted accumulated message
/// isn't of that size.
#[derive(Debug, Clone)]
pub struct MessageReadBuffer {
    message_buf: Vec<u8>,
    message_len: usize,
    /// Bytes read for current message.
    read_bytes: usize,
    chunk_reader: ChunkReadBuffer,
}

impl MessageReadBuffer {
    pub fn new() -> Self {
        Self {
            message_buf: vec![],
            message_len: 0,
            read_bytes: 0,
            chunk_reader: ChunkReadBuffer::new(),
        }
    }

    pub fn is_finished(&self) -> bool {
        self.message_len > 0 && self.message_buf.len() >= self.message_len
    }

    /// Returns if more might be available to be read.
    fn _read_from<R: Read>(
        &mut self,
        reader: &mut R,
        crypto: &mut PeerCrypto,
    ) -> Result<PeerMessageResponse, ReadMessageError> {
        loop {
            self.chunk_reader.read_from(reader)?;
            // TODO: stop reading if message_len < message_buf.len() + chunk_expected_len

            if let Some(chunk) = self.chunk_reader.take_ref_if_ready() {
                let mut decrypted = crypto.decrypt(&chunk.content())?;

                if self.message_len == 0 {
                    self.message_len = PeerMessageResponse::size_from_chunk(&decrypted)?;
                }
                self.message_buf.append(&mut decrypted);

                if self.is_finished() {
                    return self.take_and_decode();
                }
            }
        }
    }

    pub fn read_from<R: Read>(
        &mut self,
        reader: &mut R,
        crypto: &mut PeerCrypto,
    ) -> Result<PeerMessageResponse, ReadMessageError> {
        let mut counted_reader = ReadCounter::new(reader);
        let result = self._read_from(&mut counted_reader, crypto);
        self.read_bytes += counted_reader.result();
        result
    }

    fn take_and_decode(&mut self) -> Result<PeerMessageResponse, ReadMessageError> {
        let result = PeerMessageResponse::from_bytes(&self.message_buf);
        let read_bytes = self.read_bytes;

        self.clear();
        let mut resp = result?;
        resp.set_size_hint(read_bytes);
        Ok(resp)
    }

    pub fn clear(&mut self) {
        self.message_buf.clear();
        self.message_len = 0;
        self.read_bytes = 0;
        self.chunk_reader.clear();
    }
}

/// Counts exactly how many bytes were read from the stream.
struct ReadCounter<'a, R> {
    reader: &'a mut R,
    read_bytes: usize,
}

impl<'a, R> ReadCounter<'a, R> {
    pub fn new(reader: &'a mut R) -> Self {
        Self {
            reader,
            read_bytes: 0,
        }
    }

    pub fn result(self) -> usize {
        self.read_bytes
    }
}

impl<'a, R: Read> Read for ReadCounter<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let size = self.reader.read(buf)?;
        self.read_bytes += size;
        Ok(size)
    }
}
