use anyhow::{anyhow, Error};
use anyhow::Result;
use bytes::{Buf, BufMut, BytesMut};
use std::fmt::Formatter;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use tezos_context::channel::ContextActionMessage;

use crypto::hash::HashType;
use serde::{Deserialize, Serialize};

const HEADER_LEN: usize = 44;
const BLOCK_HASH_HEADER_LEN: usize = 32;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub block_level: u32,
    pub block_hash_hex: String,
    pub block_hash: [u8; BLOCK_HASH_HEADER_LEN],
    pub predecessor: [u8; BLOCK_HASH_HEADER_LEN],
}

impl Block {
    pub fn new(block_level: u32, raw_block_hash: Vec<u8>, raw_predecessor: Vec<u8>) -> Self {
        let block_hash_hex = hex::encode(&raw_block_hash);
        let mut predecessor = [0_u8; BLOCK_HASH_HEADER_LEN];
        copy_hash_to_slice(raw_predecessor, &mut predecessor);
        let mut block_hash = [0_u8; BLOCK_HASH_HEADER_LEN];
        copy_hash_to_slice(raw_block_hash, &mut block_hash);
        Block {
            block_level,
            block_hash_hex,
            predecessor,
            block_hash,
        }
    }
}

fn copy_hash_to_slice(from: Vec<u8>, to: &mut [u8; BLOCK_HASH_HEADER_LEN]) {
    let mut bytes = BytesMut::with_capacity(BLOCK_HASH_HEADER_LEN);
    bytes.put_slice(&from);
    let _ = bytes.reader().read_exact(to);
}

#[derive(Clone, Copy, Debug)]
pub struct ActionsFileHeader {
    pub current_block_hash: [u8; BLOCK_HASH_HEADER_LEN],
    pub block_height: u32,
    pub actions_count: u32,
    pub block_count: u32,
}

impl std::fmt::Display for ActionsFileHeader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut formatter: String = String::new();
        formatter.push_str(&format!(
            "{:<24}{}\n",
            "Block Hash:",
            HashType::BlockHash.hash_to_b58check(&self.current_block_hash)
        ));
        formatter.push_str(&format!("{:<24}{}\n", "Block Height:", self.block_height));
        formatter.push_str(&format!("{:<24}{}\n", "Block Count:", self.block_count));
        formatter.push_str(&format!("{:<24}{}", "Actions Count:", self.actions_count));
        writeln!(f, "{}", formatter)
    }
}

impl From<[u8; HEADER_LEN]> for ActionsFileHeader {
    fn from(v: [u8; HEADER_LEN]) -> Self {
        let mut bytes = BytesMut::with_capacity(v.len());
        bytes.put_slice(&v);
        let block_height = bytes.get_u32();
        let actions_count = bytes.get_u32();
        let block_count = bytes.get_u32();
        let mut hash = [0_u8; BLOCK_HASH_HEADER_LEN];
        let _ = bytes.reader().read_exact(&mut hash);

        ActionsFileHeader {
            block_height,
            actions_count,
            block_count,
            current_block_hash: hash,
        }
    }
}

impl ActionsFileHeader {
    fn to_vec(&self) -> Vec<u8> {
        let mut bytes = BytesMut::with_capacity(HEADER_LEN);
        bytes.put_u32(self.block_height);
        bytes.put_u32(self.actions_count);
        bytes.put_u32(self.block_count);
        bytes.put_slice(&self.current_block_hash);
        bytes.to_vec()
    }
}

pub struct ActionsFileReader {
    header: ActionsFileHeader,
    cursor: u64,
    reader: BufReader<File>,
}

impl ActionsFileReader {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = OpenOptions::new()
            .write(false)
            .create(false)
            .read(true)
            .open(path)?;
        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::Start(0))?;
        let mut h = [0_u8; HEADER_LEN];
        reader.read_exact(&mut h)?;
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

    pub fn fetch_header(&mut self) -> Result<ActionsFileHeader> {
        self.reader.seek(SeekFrom::Start(0))?;
        let mut h = [0_u8; HEADER_LEN];
        self.reader.read_exact(&mut h)?;
        self.header = ActionsFileHeader::from(h);
        Ok(self.header())
    }
}

impl Iterator for ActionsFileReader {
    type Item = (Block, Vec<ContextActionMessage>);

    /// Return a tuple of a block and list action in the block
    fn next(&mut self) -> Option<Self::Item> {
        self.cursor = match self.reader.seek(SeekFrom::Start(self.cursor)) {
            Ok(c) => c,
            Err(_) => {
                return None;
            }
        };
        let mut h = [0_u8; 4];
        match self.reader.read_exact(&mut h){
            Ok(_) => {}
            Err(_) => {
                return None
            }
        }
        let content_len = u32::from_be_bytes(h);
        if content_len <= 0 {
            return None;
        }
        let mut b = BytesMut::with_capacity(content_len as usize);
        unsafe { b.set_len(content_len as usize) }
        let _ = self.reader.read_exact(&mut b);

        let reader = snap::read::FrameDecoder::new(b.reader());

        let item = match bincode::deserialize_from::<_, (Block, Vec<ContextActionMessage>)>(reader)
        {
            Ok(item) => item,
            Err(_) => {
                return None;
            }
        };
        self.cursor += h.len() as u64 + content_len as u64;
        Some(item)
    }
}

/// # ActionFileWriter
///
/// writes block and list actions to file in `path`
pub struct ActionsFileWriter {
    header: ActionsFileHeader,
    file: File,
}

impl ActionsFileWriter {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .read(true)
            .open(path)?;

        let mut h = [0_u8; HEADER_LEN];

        let mut reader = BufReader::new(file.try_clone()?);
        reader.seek(SeekFrom::Start(0))?;
        reader.read_exact(&mut h) ;
        let header = ActionsFileHeader::from(h);
        Ok(ActionsFileWriter { file, header })
    }

    pub fn header(&self) -> ActionsFileHeader {
        self.header
    }
}


impl ActionsFileWriter {
    pub fn update(&mut self, block: Block, actions: Vec<ContextActionMessage>) -> Result<u32> {
        let block_level = block.block_level;
        let actions_count = actions.len() as u32;
        let block_hash = block.block_hash;
        self._fetch_header()?;
        // Check if currently saved block precedes the incoming block
        if block.predecessor != self.header.current_block_hash && self.header.block_count > 0 {
            return Err(anyhow!("Block out of sequence"));
        }

        let mut out = Vec::new();
        let writer = snap::write::FrameEncoder::new(&mut out);
        bincode::serialize_into(writer, &(block, actions))?;

        // Writes the header if its not already set
        if self.header.block_count <= 0 {
            let header_bytes = self.header.to_vec();
            self.file.seek(SeekFrom::Start(0))?;
            self.file.write(&header_bytes)?;
        }
        self._update(&out)?;
        self._update_header(block_level, actions_count, block_hash)?;
        Ok(block_level + 1)
    }

    fn _update_header(
        &mut self,
        block_level: u32,
        actions_count: u32,
        block_hash: [u8; BLOCK_HASH_HEADER_LEN],
    ) -> Result<()> {
        self.header.block_height = block_level;
        self.header.actions_count += actions_count;
        self.header.block_count += 1;
        self.header.current_block_hash = block_hash;

        let header_bytes = self.header.to_vec();
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write(&header_bytes)?;
        Ok(())
    }

    fn _fetch_header(&mut self) -> Result<()>{
        self.file.seek(SeekFrom::Start(0))?;
        let mut h = [0_u8; HEADER_LEN];
        self.file.read_exact(&mut h)?;
        self.header = ActionsFileHeader::from(h);
        Ok(())
    }

    pub fn _update(&mut self, data: &[u8]) -> Result<()>{
        self.file.seek(SeekFrom::End(0))?;
        let header = (data.len() as u32).to_be_bytes();
        let mut dt = vec![];
        dt.extend_from_slice(&header);
        dt.extend_from_slice(data);
        self.file.write(dt.as_slice())?;
        Ok(())
    }
}
