use std::convert::TryInto;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::commit_log::error::TezedgeCommitLogError;
use crate::commit_log::reader::Reader;
use crate::commit_log::writer::Writer;

pub type CommitLogRef = Arc<RwLock<CommitLog>>;

pub mod error;
mod reader;
mod writer;
pub mod commit_log;

const INDEX_FILE_NAME: &str = "table.index";
const DATA_FILE_NAME: &str = "table.data";

const TH_LENGTH: usize = 16;

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
        let data = &self.buf[self.acc..(self.acc + index.data_length as usize)];
        self.acc += index.data_length as usize;
        self.cursor += 1;
        Some(data.to_vec())
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Index {
    pub position: u64,
    pub data_length: u64,
}

impl Index {
    fn from_buf(buf: &[u8]) -> Result<Self, TezedgeCommitLogError> {
        if buf.len() != TH_LENGTH {
            return Err(TezedgeCommitLogError::IndexLengthError);
        }
        let mut buf = buf.to_vec();
        let position_raw_bytes: Vec<_> = buf.drain(..8).collect();
        let uncompressed_data_length_raw_bytes: Vec<_> = buf.drain(..).collect();
        let position = u64::from_be_bytes(
            position_raw_bytes
                .as_slice()
                .try_into()
                .map_err(|_| TezedgeCommitLogError::TryFromSliceError)?,
        );

        let data_length = u64::from_be_bytes(
            uncompressed_data_length_raw_bytes
                .as_slice()
                .try_into()
                .map_err(|_| TezedgeCommitLogError::TryFromSliceError)?,
        );
        Ok(Self {
            position,
            data_length,
        })
    }
    fn new(position: u64, data_length: u64) -> Self {
        Self {
            position,
            data_length,
        }
    }
    fn to_vec(&self) -> Vec<u8> {
        let mut buf = vec![];
        buf.extend_from_slice(&self.position.to_be_bytes());
        buf.extend_from_slice(&self.data_length.to_be_bytes());
        return buf;
    }
}

pub struct CommitLog {
    writer: Writer,
    path: PathBuf,
}

impl CommitLog {
    pub fn new<P: AsRef<Path>>(log_dir: P) -> Result<Self, TezedgeCommitLogError> {
        let writer = Writer::new(log_dir.as_ref())?;
        Ok(Self {
            writer,
            path: log_dir.as_ref().to_path_buf(),
        })
    }
    #[inline]
    pub fn append_msg<B: AsRef<[u8]>>(&mut self, payload: B) -> Result<u64, TezedgeCommitLogError> {
        let offset = self.writer.write(payload.as_ref())?;
        Ok(offset)
    }

    #[inline]
    pub fn read(&self, from: usize, limit: usize) -> Result<MessageSet, TezedgeCommitLogError> {
        let reader = Reader::new(&self.path)?;
        return reader.range(from, limit);
    }

    pub fn flush(&mut self) -> Result<(), TezedgeCommitLogError> {
        self.writer.flush()?;
        return Ok(());
    }
}
