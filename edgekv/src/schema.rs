// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crc::{Crc, CRC_32_CKSUM};
use std::io::Read;
use time::OffsetDateTime;
pub const CRC_CKSUM: Crc<u32> = Crc::<u32>::new(&CRC_32_CKSUM);
use crate::Result;

#[derive(Debug, Clone, PartialOrd, PartialEq)]
pub struct DataEntry {
    crc: u32,
    timestamp: i64,
    key_size: u64,
    value_size: u64,
    key: Vec<u8>,
    value: Vec<u8>,
}

pub trait Encoder {
    fn encode(&self) -> Vec<u8>;
}

pub trait Decoder {
    fn decode<R: Read>(rdr: &mut R) -> Result<Self>
    where
        Self: Sized;
}

impl Encoder for DataEntry {
    fn encode(&self) -> Vec<u8> {
        let content = self.encode_content();
        let crc = CRC_CKSUM.checksum(&content);
        let mut buf = vec![];
        buf.extend_from_slice(&crc.to_be_bytes());
        buf.extend_from_slice(&content);
        buf
    }
}

impl Decoder for DataEntry {
    fn decode<R: Read>(rdr: &mut R) -> Result<Self>
    where
        Self: Sized,
    {
        let mut out = Self {
            crc: 0,
            timestamp: 0,
            key_size: 0,
            value_size: 0,
            key: vec![],
            value: vec![],
        };
        let mut raw_crc_bytes = [0_u8; 4];
        let mut raw_timestamp_bytes = [0_u8; 8];
        let mut raw_key_size_bytes = [0_u8; 8];
        let mut raw_value_size_bytes = [0_u8; 8];

        rdr.read_exact(&mut raw_crc_bytes)?;
        rdr.read_exact(&mut raw_timestamp_bytes)?;
        rdr.read_exact(&mut raw_key_size_bytes)?;
        rdr.read_exact(&mut raw_value_size_bytes)?;

        out.crc = u32::from_be_bytes(raw_crc_bytes);
        out.timestamp = i64::from_be_bytes(raw_timestamp_bytes);
        out.key_size = u64::from_be_bytes(raw_key_size_bytes);
        out.value_size = u64::from_be_bytes(raw_value_size_bytes);

        let mut raw_key_bytes = vec![0_u8; out.key_size as usize];
        let mut raw_value_bytes = vec![0_u8; out.value_size as usize];

        // TODO - TE-721: handle these errors
        rdr.read_exact(&mut raw_key_bytes)?;
        rdr.read_exact(&mut raw_value_bytes)?;

        out.key = raw_key_bytes;
        out.value = raw_value_bytes;

        Ok(out)
    }
}

impl DataEntry {
    pub fn new(key: Vec<u8>, value: Vec<u8>) -> Self {
        let timestamp = OffsetDateTime::now_utc().unix_timestamp();
        let key_size = key.len() as u64;
        let value_size = value.len() as u64;

        Self {
            crc: 0,
            timestamp,
            key_size,
            value_size,
            key,
            value,
        }
    }

    pub fn check_crc(&self) -> bool {
        self.crc == CRC_CKSUM.checksum(&self.encode_content())
    }

    fn encode_content(&self) -> Vec<u8> {
        let mut buf = vec![];
        buf.extend_from_slice(&self.timestamp.to_be_bytes());
        buf.extend_from_slice(&self.key_size.to_be_bytes());
        buf.extend_from_slice(&self.value_size.to_be_bytes());
        buf.extend_from_slice(&self.key);
        buf.extend_from_slice(&self.value);
        buf
    }

    pub fn key(&self) -> Vec<u8> {
        self.key.to_owned()
    }
    pub fn value(&self) -> Vec<u8> {
        self.value.to_owned()
    }
}

pub struct HintEntry {
    crc: u32,
    timestamp: i64,
    key_size: u64,
    value_size: u64,
    data_entry_position: u64,
    key: Vec<u8>,
}

impl HintEntry {
    pub fn from(entry: &DataEntry, position: u64) -> Self {
        Self {
            crc: 0,
            timestamp: entry.timestamp,
            key_size: entry.key_size,
            value_size: entry.value_size,
            data_entry_position: position,
            key: entry.key.clone(),
        }
    }
    pub fn tombstone(key: Vec<u8>) -> Self {
        Self {
            crc: 0,
            timestamp: -1,
            key_size: key.len() as u64,
            value_size: 0,
            data_entry_position: 0,
            key,
        }
    }
    pub fn data_entry_position(&self) -> u64 {
        self.data_entry_position
    }

    pub fn is_deleted(&self) -> bool {
        self.timestamp <= 0 && self.value_size == 0 && self.data_entry_position == 0
    }

    fn encode_content(&self) -> Vec<u8> {
        let mut buf = vec![];
        buf.extend_from_slice(&self.timestamp.to_be_bytes());
        buf.extend_from_slice(&self.key_size.to_be_bytes());
        buf.extend_from_slice(&self.value_size.to_be_bytes());
        buf.extend_from_slice(&self.data_entry_position.to_be_bytes());
        buf.extend_from_slice(&self.key);
        buf
    }

    pub fn key_size(&self) -> u64 {
        self.key_size
    }
    pub fn value_size(&self) -> u64 {
        self.value_size
    }
    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }
    pub fn key(&self) -> Vec<u8> {
        self.key.to_owned()
    }

    pub fn size(&self) -> usize {
        (4 * 8) + self.key.len() + 4
    }

    pub fn check_crc(&self) -> bool {
        self.crc == CRC_CKSUM.checksum(&self.encode_content())
    }
}

impl Encoder for HintEntry {
    fn encode(&self) -> Vec<u8> {
        let content = self.encode_content();
        let crc = CRC_CKSUM.checksum(&content);
        let mut buf = vec![];
        buf.extend_from_slice(&crc.to_be_bytes());
        buf.extend_from_slice(&content);
        buf
    }
}

impl Decoder for HintEntry {
    fn decode<R: Read>(rdr: &mut R) -> Result<Self>
    where
        Self: Sized,
    {
        let mut out = Self {
            crc: 0,
            timestamp: 0,
            key_size: 0,
            value_size: 0,
            data_entry_position: 0,
            key: vec![],
        };

        let mut raw_crc_bytes = [0_u8; 4];
        let mut raw_timestamp_bytes = [0_u8; 8];
        let mut raw_key_size_bytes = [0_u8; 8];
        let mut raw_value_size_bytes = [0_u8; 8];
        let mut raw_data_entry_pos_size_bytes = [0_u8; 8];

        rdr.read_exact(&mut raw_crc_bytes)?;
        rdr.read_exact(&mut raw_timestamp_bytes)?;
        rdr.read_exact(&mut raw_key_size_bytes)?;
        rdr.read_exact(&mut raw_value_size_bytes)?;
        rdr.read_exact(&mut raw_data_entry_pos_size_bytes)?;

        out.crc = u32::from_be_bytes(raw_crc_bytes);
        out.timestamp = i64::from_be_bytes(raw_timestamp_bytes);
        out.key_size = u64::from_be_bytes(raw_key_size_bytes);
        out.value_size = u64::from_be_bytes(raw_value_size_bytes);
        out.data_entry_position = u64::from_be_bytes(raw_data_entry_pos_size_bytes);

        let mut raw_key_bytes = vec![0_u8; out.key_size as usize];
        // TODO - TE-721: handle this error
        rdr.read_exact(&mut raw_key_bytes)?;
        out.key = raw_key_bytes;

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use crate::schema::{DataEntry, Decoder, Encoder};
    use std::io::Cursor;

    #[test]
    fn decode_encode_test() {
        let rec = DataEntry::new(vec![2, 2, 3, 54, 12], vec![32, 4, 1, 32, 65, 78]);
        let e = rec.encode();
        let d = DataEntry::decode(&mut Cursor::new(e)).unwrap();
        println!("{:#?}", d);
        println!("{}", d.check_crc())
    }
}
