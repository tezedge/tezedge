// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Fail;
use rocksdb::{ColumnFamilyDescriptor, Options};

use tezos_encoding::hash::Hash;

/// Possible errors for schema
#[derive(Debug, Fail)]
pub enum SchemaError {
    #[fail(display = "Failed to encode value")]
    EncodeError,
    #[fail(display = "Failed to decode value")]
    DecodeError,
}

pub trait Codec: Sized {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError>;

    fn encode(&self) -> Result<Vec<u8>, SchemaError>;
}

pub trait Schema {
    const COLUMN_FAMILY_NAME: &'static str;

    type Key: Codec;

    type Value: Codec;

    fn cf_descriptor() -> ColumnFamilyDescriptor {
        ColumnFamilyDescriptor::new(Self::COLUMN_FAMILY_NAME, Options::default())
    }
}

impl Codec for Hash {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        Ok(bytes.to_vec())
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        Ok(self.clone())
    }
}
