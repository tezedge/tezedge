use crate::commit_log::error::TezedgeCommitLogError;
use crate::commit_log::{Index, MessageSet, DATA_FILE_NAME, INDEX_FILE_NAME, TH_LENGTH};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Error};
use std::path::{Path, PathBuf};

pub(crate) struct Reader {
    index_file: File,
    data_file: File,
}

impl Reader {
    pub(crate) fn new<P: AsRef<Path>>(dir: P) -> Result<Self, TezedgeCommitLogError> {
        if !dir.as_ref().exists() {
            return Err(TezedgeCommitLogError::PathError);
        }

        let mut index_file_path = PathBuf::new();
        index_file_path.push(dir.as_ref());
        index_file_path.push(INDEX_FILE_NAME);

        let mut data_file_path = PathBuf::new();
        data_file_path.push(dir.as_ref());
        data_file_path.push(DATA_FILE_NAME);

        let index_file = OpenOptions::new()
            .create(false)
            .write(false)
            .read(true)
            .open(index_file_path.as_path())?;

        let data_file = OpenOptions::new()
            .create(false)
            .write(false)
            .read(true)
            .open(data_file_path.as_path())?;

        let reader = Self {
            index_file,
            data_file,
        };

        Ok(reader)
    }

    pub fn indexes(&self) -> Vec<Index> {
        let mut index_file_buf_reader = BufReader::new(&self.index_file);
        match index_file_buf_reader.seek(SeekFrom::Start(0)) {
            Ok(_) => {}
            Err(_) => {
                vec![]
            }
        };
        let mut indexes = vec![];
        let mut buf = Vec::new();
        match index_file_buf_reader.read_to_end(&mut buf) {
            Ok(_) => {}
            Err(_) => {
                vec![]
            }
        };
        let header_chunks = buf.chunks_exact(TH_LENGTH);
        for chunk in header_chunks {
            let th = Index::from_buf(chunk).unwrap();
            indexes.push(th)
        }
        indexes
    }

    pub(crate) fn range(
        &self,
        from: usize,
        limit: usize,
    ) -> Result<MessageSet, TezedgeCommitLogError> {
        let indexes = self.indexes();
        if from + limit > indexes.len() {
            return Err(TezedgeCommitLogError::OutOfRange);
        }
        let mut data_file_buf_reader = BufReader::new(&self.data_file);
        let from_index = indexes[from];
        let range: Vec<_> = indexes[from..].iter().copied().take(limit).collect();
        let total_compressed_data_size = range
            .iter()
            .fold(0_u64, |acc, item| acc + item.compressed_data_length);
        let mut compressed_bytes = vec![0; total_compressed_data_size as usize];
        data_file_buf_reader.seek(SeekFrom::Start(from_index.position))?;
        data_file_buf_reader.read_exact(&mut compressed_bytes)?;

        let mut uncompressed_bytes = Vec::new();
        {
            let mut rdr = snap::read::FrameDecoder::new(compressed_bytes.as_slice());
            rdr.read_to_end(&mut uncompressed_bytes)?;
        }
        Ok(MessageSet::new(range, uncompressed_bytes))
    }
}
