use std::io::{BufReader, Read, Seek, SeekFrom};
use std::fs::{File, OpenOptions};
use std::error::Error;
use std::path::{Path};
use bytes::{BytesMut, Buf, BufMut};
// use bytes::buf::BufExt;
use std::fmt::Formatter;
// use crate::context_action_storage::ContextAction;
use serde::{Serialize, Deserialize};

const HEADER_LEN: usize = 12;
type Hash = Vec<u8>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub block_level : u32,
    pub block_hash : String
}

#[derive(Clone, Copy, Debug)]
pub struct ActionsFileHeader {
    pub block_height: u32,
    pub actions_count: u32,
    pub block_count: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ContextAction {
    Set {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        key: Vec<String>,
        value: Vec<u8>,
        value_as_json: Option<String>,
        ignored: bool,
        start_time: f64,
        end_time: f64,
    },
    Delete {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        key: Vec<String>,
        ignored: bool,
        start_time: f64,
        end_time: f64,
    },
    RemoveRecursively {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        key: Vec<String>,
        ignored: bool,
        start_time: f64,
        end_time: f64,
    },
    Copy {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        from_key: Vec<String>,
        to_key: Vec<String>,
        ignored: bool,
        start_time: f64,
        end_time: f64,
    },
    Checkout {
        context_hash: Hash,
        start_time: f64,
        end_time: f64,
    },
    Commit {
        parent_context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        new_context_hash: Hash,
        author: String,
        message: String,
        date: i64,
        parents: Vec<Vec<u8>>,
        start_time: f64,
        end_time: f64,
    },
    Mem {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        key: Vec<String>,
        value: bool,
        start_time: f64,
        end_time: f64,
    },
    DirMem {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        key: Vec<String>,
        value: bool,
        start_time: f64,
        end_time: f64,
    },
    Get {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        key: Vec<String>,
        value: Vec<u8>,
        value_as_json: Option<String>,
        start_time: f64,
        end_time: f64,
    },
    Fold {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        key: Vec<String>,
        start_time: f64,
        end_time: f64,
    },
    /// This is a control event used to shutdown IPC channel
    Shutdown,
}

impl std::fmt::Display for ActionsFileHeader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut formatter: String = String::new();
        formatter.push_str(&format!("{:<24}{}\n", "Block Height:", self.block_height));
        formatter.push_str(&format!("{:<24}{}\n", "Block Count:", self.block_count));
        formatter.push_str(&format!("{:<24}{}", "Actions Count:", self.actions_count));
        writeln!(f, "{}", formatter)
    }
}


impl From<[u8; HEADER_LEN]> for ActionsFileHeader {
    fn from(v: [u8; 12]) -> Self {
        let mut bytes = BytesMut::with_capacity(v.len());
        bytes.put_slice(&v);
        let block_height = bytes.get_u32();
        let actions_count = bytes.get_u32();
        let block_count = bytes.get_u32();

        ActionsFileHeader {
            block_height,
            actions_count,
            block_count,
        }
    }
}

impl ActionsFileHeader {
    fn to_vec(&self) -> Vec<u8> {
        let mut bytes = BytesMut::with_capacity(HEADER_LEN);
        bytes.put_u32(self.block_height);
        bytes.put_u32(self.actions_count);
        bytes.put_u32(self.block_count);
        bytes.to_vec()
    }
    fn new() -> Self {
        ActionsFileHeader {
            block_height: 0,
            actions_count: 0,
            block_count: 0,
        }
    }
}

/// # ActionFileReader
/// Reads actions binary file in `path`
/// ## Examples
/// ```
/// use io::ActionsFileReader;
///
/// let reader = ActionsFileReader::new("./actions.bin").unwrap();
/// println!("{}", reader.header());
/// ```

pub struct ActionsFileReader {
    header: ActionsFileHeader,
    cursor: u64,
    reader: BufReader<File>,
}


impl ActionsFileReader {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let mut file = OpenOptions::new().write(false).create(false).read(true).open(path)?;
        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::Start(0)).unwrap();
        let mut h = [0_u8; HEADER_LEN];
        reader.read_exact(&mut h).unwrap();
        let header = ActionsFileHeader::from(h);
        Ok(ActionsFileReader {
            reader,
            header,
            cursor: HEADER_LEN as u64,
        })
    }

    /// Prints header `ActionsFileHeader`
    pub fn header(&self) -> ActionsFileHeader {
        self.header
    }

    pub fn fetch_header(&mut self) -> ActionsFileHeader {
        self.reader.seek(SeekFrom::Start(0)).unwrap();
        let mut h = [0_u8; HEADER_LEN];
        self.reader.read_exact(&mut h).unwrap();
        self.header = ActionsFileHeader::from(h);
        self.header()
    }
}

impl Iterator for ActionsFileReader {
    type Item = (Block, Vec<ContextAction>);

    /// Return a tuple of a block and list action in the block
    fn next(&mut self) -> Option<Self::Item> {
        self.cursor = match self.reader.seek(SeekFrom::Start(self.cursor)) {
            Ok(c) => {
                c
            }
            Err(_) => {
                return None;
            }
        };
        let mut h = [0_u8; 4];
        //stops iteration when content length size cannot be read correctly
        match self.reader.read_exact(&mut h) {
            Ok(_) => {}
            Err(_) => {
                return None
            }
        };
        let content_len = u32::from_be_bytes(h);
        if content_len <= 0 {
            return None;
        }
        let mut b = BytesMut::with_capacity(content_len as usize);
        unsafe { b.set_len(content_len as usize) }
        
        //stops iteration when content length doesnt match exactly
        match self.reader.read_exact(&mut b){
            Ok(_) => {}
            Err(_) => {
                return None
            }
        };
        

        let mut reader = snap::read::FrameDecoder::new(b.reader());

        let item = match bincode::deserialize_from::<_, (Block, Vec<ContextAction>)>(reader) {
            Ok(item) => {
                item
            }
            Err(_) => {
                return None;
            }
        };
        self.cursor += h.len() as u64 + content_len as u64;
        Some(item)
    }
}
