// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use failure::Fail;
use rocksdb::{ColumnFamily, DBIterator, DB};
use networking::p2p::binary_message::BinaryMessage;
use failure::_core::marker::PhantomData;

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

    fn encode(&self) -> Result<Vec<[u8]>, SchemaError>;
}

pub trait Schema {
    const COLUMN_FAMILY: &'static str;

    type Key: Codec;

    type Value: Codec;
}


mod test {
    use super::*;
    use networking::p2p::encoding::prelude::*;
    use tezos_encoding::hash::{HashRef, Hash};

    struct OperationStorageItem;

    impl Codec for Hash {

        fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
            unimplemented!()
        }

        fn encode(&self) -> Result<Vec<[u8]>, SchemaError> {
            unimplemented!()
        }
    }

    impl Codec for OperationsForBlocksMessage {
        fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
            unimplemented!()
        }

        fn encode(&self) -> Result<Vec<[u8]>, SchemaError> {
            unimplemented!()
        }
    }

    impl Schema for OperationStorageItem {
        const COLUMN_FAMILY_NAME: &'static str = "ops_for_blks";
        type Key = Hash;
        type Value = OperationsForBlocksMessage;
    }


    fn how_to() {

    }
}

