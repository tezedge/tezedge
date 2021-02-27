use std::convert::TryInto;
use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::commit_log::error::TezedgeCommitLogError;
use crate::commit_log::reader::Reader;
use crate::commit_log::writer::Writer;

pub type CommitLogRef = Arc<RwLock<CommitLog>>;

pub mod error;
mod reader;
mod writer;

const INDEX_FILE_NAME: &str = "table.index";
const DATA_FILE_NAME: &str = "table.data";

const TH_LENGTH: usize = 24;

pub type Message = Vec<u8>;

pub struct MessageSet {
    cursor: usize,
    indexes: Vec<Index>,
    acc: usize,
    buf: Vec<u8>,
}

impl MessageSet {
    fn new(indexes: Vec<Index>, buf: Vec<u8>) -> Self {
        Self {
            cursor: 0,
            indexes,
            acc: 0,
            buf,
        }
    }
}

impl Iterator for MessageSet {
    type Item = Message;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.indexes.get(self.cursor)?;
        let data = &self.buf[self.acc..(self.acc + index.uncompressed_data_length as usize)];
        self.acc += index.uncompressed_data_length as usize;
        self.cursor += 1;
        Some(data.to_vec())
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Index {
    pub position: u64,
    pub compressed_data_length: u64,
    pub uncompressed_data_length: u64,
}

impl Index {
    fn from_buf(buf: &[u8]) -> Result<Self, TezedgeCommitLogError> {
        if buf.len() != TH_LENGTH {
            return Err(TezedgeCommitLogError::IndexLengthError);
        }
        let mut buf = buf.to_vec();
        let position_raw_bytes: Vec<_> = buf.drain(..8).collect();
        let compressed_data_length_raw_bytes: Vec<_> = buf.drain(..8).collect();
        let uncompressed_data_length_raw_bytes: Vec<_> = buf.drain(..).collect();
        let position = u64::from_be_bytes(
            position_raw_bytes
                .as_slice()
                .try_into()
                .map_err(|_| TezedgeCommitLogError::TryFromSliceError)?,
        );
        let compressed_data_length = u64::from_be_bytes(
            compressed_data_length_raw_bytes
                .as_slice()
                .try_into()
                .map_err(|_| TezedgeCommitLogError::TryFromSliceError)?,
        );
        let uncompressed_data_length = u64::from_be_bytes(
            uncompressed_data_length_raw_bytes
                .as_slice()
                .try_into()
                .map_err(|_| TezedgeCommitLogError::TryFromSliceError)?,
        );
        Ok(Self {
            position,
            compressed_data_length,
            uncompressed_data_length,
        })
    }
    fn new(position: u64, compressed_data_length: u64, uncompressed_data_length: u64) -> Self {
        Self {
            position,
            compressed_data_length,
            uncompressed_data_length,
        }
    }
    fn to_vec(&self) -> Vec<u8> {
        let mut buf = vec![];
        buf.extend_from_slice(&self.position.to_be_bytes());
        buf.extend_from_slice(&self.compressed_data_length.to_be_bytes());
        buf.extend_from_slice(&self.uncompressed_data_length.to_be_bytes());
        return buf;
    }
}

pub struct CommitLog {
    reader: Reader,
    writer: Writer,
}

impl CommitLog {
    pub fn new<P: AsRef<Path>>(log_dir: P) -> Result<Self, TezedgeCommitLogError> {
        let writer = Writer::new(log_dir.as_ref())?;
        let reader = Reader::new(log_dir.as_ref())?;
        Ok(Self { reader, writer })
    }
    #[inline]
    pub fn append_msg<B: AsRef<[u8]>>(&mut self, payload: B) -> Result<u64, TezedgeCommitLogError> {
        let offset = self.writer.write(payload.as_ref())?;
        Ok(offset)
    }

    #[inline]
    pub fn read(&self, from: usize, limit: usize) -> Result<MessageSet, TezedgeCommitLogError> {
        return self.reader.range(from, limit);
    }

    pub fn flush(&mut self) -> Result<(), TezedgeCommitLogError> {
        self.writer.flush()?;
        return Ok(());
    }
}
