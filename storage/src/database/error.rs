use crate::persistent::SchemaError;
use crypto::hash::FromBytesError;
use failure::Fail;
use std::io;

/// Possible errors for schema
#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Guard Poison {} ", error)]
    GuardPoison { error: String },
    #[fail(display = "Value Missing {} ", error)]
    NutosError { error: String },
    #[fail(display = "Missing sub tree {} ", error)]
    MissingSubTree { error: String },
    #[fail(display = "SledDB error: {}", error)]
    SledDBError { error: sled::Error },
    #[fail(display = "I/O error {}", error)]
    IOError { error: io::Error },
    #[fail(display = "Schema error: {}", error)]
    SchemaError { error: SchemaError },
    #[fail(display = "Column name required")]
    ColumnRequired,
    #[fail(display = "Hash encode error : {}", error)]
    HashEncodeError { error: FromBytesError },
    #[fail(display = "Database incompatibility {}", name)]
    DatabaseIncompatibility { name: String },
    #[fail(display = "RocksDB error: {}", error)]
    RocksDBError { error: rocksdb::Error },
    #[fail(display = "Column family {} is missing", name)]
    MissingColumnFamily { name: &'static str },
}

impl From<SchemaError> for Error {
    fn from(error: SchemaError) -> Self {
        Error::SchemaError { error }
    }
}
impl From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
        Error::SledDBError { error }
    }
}
impl From<rocksdb::Error> for Error {
    fn from(error: rocksdb::Error) -> Self {
        Error::RocksDBError { error }
    }
}
impl From<FromBytesError> for Error {
    fn from(error: FromBytesError) -> Self {
        Error::HashEncodeError { error }
    }
}
