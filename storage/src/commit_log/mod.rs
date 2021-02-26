use std::convert::TryInto;
use crate::commit_log::error::TezedgeCommitLogError;
use std::path::{Path, PathBuf};
use crate::commit_log::reader::Reader;
use crate::commit_log::writer::Writer;

mod reader;
pub mod error;
mod writer;

const INDEX_FILE_NAME: &str = "table.index";
const DATA_FILE_NAME: &str = "table.data";

const TH_LENGTH: usize = 16;

pub type Message = Vec<u8>;

#[derive(Debug)]
pub struct Index {
    pub position: u64,
    pub data_length: u64,
}


impl Index {
    fn from_buf(buf: &[u8]) -> Result<Self, TezedgeCommitLogError> {
        if buf.len() != TH_LENGTH {
            return Err(TezedgeCommitLogError::IndexLengthError)
        }
        let mut buf = buf.to_vec();
        let td_position_raw_bytes: Vec<_> = buf.drain(..8).collect();
        let td_length_raw_bytes: Vec<_> = buf.drain(..).collect();
        let td_position = u64::from_be_bytes(td_position_raw_bytes.as_slice()
            .try_into().map_err(|_| { TezedgeCommitLogError::TryFromSliceError })?);
        let td_length = u64::from_be_bytes(td_length_raw_bytes.as_slice()
            .try_into().map_err(|_| { TezedgeCommitLogError::TryFromSliceError })?);
        Ok(Self { position: td_position, data_length: td_length })
    }
    fn new(pos: u64, len: u64) -> Self {
        Self {
            position: pos,
            data_length: len,
        }
    }
    fn to_vec(&self) -> Vec<u8> {
        let mut buf = vec![];
        for b in &self.position.to_be_bytes() {
            buf.push(*b)
        }
        for b in &self.data_length.to_be_bytes() {
            buf.push(*b)
        }
        return buf;
    }
}

pub struct CommitLog {
    path : PathBuf
}

impl CommitLog {

    pub fn new<P: AsRef<Path>>(log_dir: P) -> Self {
        Self {
            path: log_dir.as_ref().to_path_buf()
        }
    }
    #[inline]
    pub fn append_msg<B: AsRef<[u8]>>(&mut self, payload: B) -> Result<u64, TezedgeCommitLogError> {
        let mut writer = Writer::new(&self.path)?;
        let offset = writer.write(payload.as_ref())?;
        Ok(offset)
    }

    #[inline]
    pub fn read(&self, from: usize, limit: usize) -> Result<Vec<Message>, TezedgeCommitLogError> {
        let mut reader = Reader::new(&self.path)?;
        return reader.range(from, limit);
    }

    pub fn iter(&mut self) -> Result<CommitLogIterator, TezedgeCommitLogError> {
        let reader = Reader::new(&self.path)?;
        Ok(CommitLogIterator::new(reader))
    }

    pub fn flush(&mut self) -> Result<(), TezedgeCommitLogError> {
        let mut writer = Writer::new(&self.path)?;
        return writer.flush();
    }
}

pub struct CommitLogIterator{
    cursor: u64,
    reader: Reader,
}

impl CommitLogIterator {
    fn new(reader: Reader) -> Self {
        Self {
            cursor: 0,
            reader,
        }
    }
}

impl Iterator for CommitLogIterator {
    type Item = Message;

    fn next(&mut self) -> Option<Self::Item> {
        if self.reader.indexes.get(self.cursor as usize).is_none() {
            return None;
        }
        match self.reader.read_at(self.cursor as usize) {
            Ok(m) => {
                self.cursor += 1;
                Some(m)
            }
            Err(_) => {
                None
            }
        }
    }
}
