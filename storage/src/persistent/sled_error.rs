use failure::Fail;
use sled::Error;
use sled::transaction::{TransactionError};
use sled::CompareAndSwapError;

#[derive(Debug, Fail)]
pub enum SledError {
    #[fail(display = "{}", error)]
    DBError { error: Error },
    #[fail(display = "{}", error)]
    CompareAndSwapError { error: CompareAndSwapError },
    #[fail(display = "{}", error)]
    TransactionError { error: TransactionError },
}

impl From<Error> for SledError {
    fn from(error: Error) -> Self {
        SledError::DBError { error }
    }
}

impl From<CompareAndSwapError> for SledError {
    fn from(error: CompareAndSwapError) -> Self {
        SledError::CompareAndSwapError { error }
    }
}

impl From<TransactionError> for SledError {
    fn from(error: TransactionError) -> Self {
        SledError::TransactionError { error }
    }
}

impl slog::Value for SledError {
    fn serialize(&self, _record: &slog::Record, key: slog::Key, serializer: &mut dyn slog::Serializer) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}
