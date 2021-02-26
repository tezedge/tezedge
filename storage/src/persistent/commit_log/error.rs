use failure::Fail;
use std::io;
use std::array::TryFromSliceError;
use std::convert::TryFrom;

#[derive(Debug, Fail)]
pub enum TezedgeCommitLogError {
    #[fail(display = "Failed to read record")]
    ReadError,
    #[fail(display = "Failed to located path")]
    PathError,
    #[fail(display = "Commit log I/O error {}", error)]
    IOError { error: io::Error },

    #[fail(display = "Index out of range")]
    OutOfRange,

    #[fail(display = "Message length cannot be converted to u64")]
    MessageLengthError,

    #[fail(display = "Commit log mode set to read-only")]
    WriteFailed,

    #[fail(display = "Error converting slice")]
    TryFromSliceError,

    #[fail(display = "Index length error")]
    IndexLengthError,
}

pub enum ReadError {
    CorruptLog
}


impl From<io::Error> for TezedgeCommitLogError {
    fn from(error: io::Error) -> Self {
        TezedgeCommitLogError::IOError { error }
    }
}
