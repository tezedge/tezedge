use std::io;
use failure::Fail;
use crate::persistent::SchemaError;
use crypto::hash::FromBytesError;

/// Possible errors for schema
#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Guard Poison {} ", error)]
    GuardPoison { error: String },
    #[fail(display = "Missing sub tree {} ", error)]
    MissingSubTree { error: String },
    #[fail(display = "SledDB error: {}", error)]
    SledDBError { error: sled::Error },
    #[fail(display = "I/O error {}", error)]
    IOError { error: io::Error },
    #[fail(display = "Schema error: {}", error)]
    SchemaError { error: SchemaError },
    #[fail(display = "Hash encode error : {}", error)]
    HashEncodeError { error: FromBytesError },
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
impl From<FromBytesError> for Error {
    fn from(error: FromBytesError) -> Self {
        Error::HashEncodeError { error }
    }
}

