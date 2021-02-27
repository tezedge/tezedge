use std::io::{BufReader, Seek, SeekFrom, Read};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use crate::persistent::commit_log::{TH_LENGTH, Index, INDEX_FILE_NAME, DATA_FILE_NAME, MessageSet};
use crate::persistent::commit_log::error::TezedgeCommitLogError;


pub(crate) struct Reader {
    pub(crate) indexes : Vec<Index>,
    index_file: BufReader<File>,
    data_file : BufReader<File>
}

impl Reader {
    pub(crate) fn new<P: AsRef<Path>>(dir: P) -> Result<Self, TezedgeCommitLogError> {
        if !dir.as_ref().exists() {
            return Err(TezedgeCommitLogError::PathError)
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

        let mut reader = Self {
            indexes: vec![],
            index_file : BufReader::new(index_file),
            data_file: BufReader::new(data_file),
        };
        reader.update();

        Ok( reader)
    }

    pub(crate) fn update(&mut self){
        self.indexes = self.read_indexes()
    }

    fn read_indexes(&mut self) -> Vec<Index>{
        let mut indexes = vec![];
        match self.index_file.seek(SeekFrom::Start(0)) {
            Ok(_) => {}
            Err(_) => {
                return indexes
            }
        };
        let mut buf = Vec::new();
        match self.index_file.read_to_end(&mut buf){
            Ok(_) => {}
            Err(_) => {
                return indexes
            }
        };
        let header_chunks = buf.chunks_exact(TH_LENGTH);
        for chunk in header_chunks {
            let th = Index::from_buf(chunk).unwrap();
            indexes.push(th)
        }
        indexes
    }

    pub(crate) fn read_at(&mut self, index : usize) -> Result<Vec<u8>, TezedgeCommitLogError> {
        let indexes = &self.indexes;
        let index = match indexes.get(index) {
            None => {
                return Ok(vec![])
            }
            Some(index) => {
                index
            }
        };
        let mut encode_message = vec![0;index.compressed_data_length as usize];
        self.data_file.seek(SeekFrom::Start(index.position))?;
        self.data_file.read(&mut encode_message)?;

        let mut decoded_message = vec![];
        {
            let mut rdr = snap::read::FrameDecoder::new(encode_message.as_slice());
            rdr.read_to_end(&mut decoded_message)?;
        }
        Ok(decoded_message)
    }


    pub(crate) fn range(&mut self, from : usize, limit : usize) -> Result<MessageSet, TezedgeCommitLogError> {

        let indexes = &self.indexes;
        if from + limit > indexes.len() {
            return Err(TezedgeCommitLogError::OutOfRange)
        }
        let from_index = indexes[from];
        let range: Vec<_> = indexes[from..].iter().map(|i| i.clone()).take(limit).collect();
        let total_compressed_data_size = range.iter().fold(0_u64, |acc, item| {
            acc + item.compressed_data_length
        });
        let mut compressed_bytes = vec![0; total_compressed_data_size as usize];
        self.data_file.seek(SeekFrom::Start(from_index.position))?;
        self.data_file.read(&mut compressed_bytes)?;

        let mut uncompressed_bytes = Vec::new();
        {
            let mut rdr = snap::read::FrameDecoder::new(compressed_bytes.as_slice());
            rdr.read_to_end(&mut uncompressed_bytes)?;
        }
        Ok(MessageSet::new(range, uncompressed_bytes))
    }


}