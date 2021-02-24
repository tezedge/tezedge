use std::convert::TryInto;
use crate::commit_log::error::TezedgeCommitLogError;
use std::path::Path;
use crate::commit_log::reader::Reader;
use crate::commit_log::writer::Writer;

mod reader;
mod error;
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

enum CommitLogMode {
    ReadOnly(Reader),
    ReadWrite(Reader, Writer),
}

pub struct CommitLogConfig {
    mode: CommitLogMode
}

impl CommitLogConfig {
    pub fn open_for_read_only<P: AsRef<Path>>(log_dir: P) -> Result<Self, TezedgeCommitLogError> {
        Ok(Self {
            mode: CommitLogMode::ReadOnly(Reader::new(log_dir)?)
        })
    }
    pub fn open<P: AsRef<Path>>(log_dir: P) -> Result<Self, TezedgeCommitLogError> {
        let writer = Writer::new(&log_dir)?;
        let reader = Reader::new(log_dir)?;
        Ok(Self {
            mode: CommitLogMode::ReadWrite(reader, writer)
        })
    }
}

struct CommitLog {
    reader: Reader,
    writer: Option<Writer>,
}

impl CommitLog {
    pub fn new_from_config(config: CommitLogConfig) -> Self {
        match config.mode {
            CommitLogMode::ReadOnly(mut r) => {
                Self {
                    reader: r,
                    writer: None,
                }
            }
            CommitLogMode::ReadWrite(mut r, w) => {
                Self {
                    reader: r,
                    writer: Some(w),
                }
            }
        }
    }

    pub fn new<P: AsRef<Path>>(log_dir: P) -> Result<Self, TezedgeCommitLogError> {
        Ok(Self::new_from_config(CommitLogConfig::open(log_dir)?))
    }
    #[inline]
    pub fn append_msg<B: AsRef<[u8]>>(&mut self, payload: B) -> Result<(), TezedgeCommitLogError> {
        if let Some(writer) = &mut self.writer {
            writer.write(payload.as_ref())?;
            self.reader.update();
        }
        Ok(())
    }

    #[inline]
    pub fn read(&mut self, from: usize, limit: usize) -> Result<Vec<Message>, TezedgeCommitLogError> {
        return self.reader.range(from, limit);
    }

    pub fn iter(&mut self) -> CommitLogIterator {
        CommitLogIterator::new(&mut self.reader)
    }

    pub fn flush(&mut self) -> Result<(), TezedgeCommitLogError> {
        if let Some(writer) = &mut self.writer {
            return writer.flush();
        }
        Err(TezedgeCommitLogError::WriteFailed)
    }
}

struct CommitLogIterator<'a> {
    cursor: u64,
    reader: &'a mut Reader,
}

impl<'a> CommitLogIterator<'a> {
    fn new(reader: &'a mut Reader) -> Self {
        Self {
            cursor: 0,
            reader,
        }
    }
}

impl<'a> Iterator for CommitLogIterator<'a> {
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
