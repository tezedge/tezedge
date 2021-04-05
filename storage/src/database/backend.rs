use crate::database::error::Error;
use crate::Direction;
#[derive(Clone)]
pub enum BackendIteratorMode {
    Start,
    End,
    From(Vec<u8>, Direction),
}

pub trait TezedgeDatabaseBackendStore {
    fn put(&self, column: &'static str, key: Vec<u8>, value: Vec<u8>) -> Result<(), Error>;
    fn delete(&self, column: &'static str, key: Vec<u8>) -> Result<(), Error>;
    fn merge(&self, column: &'static str, key: Vec<u8>, value: Vec<u8>) -> Result<(), Error>;
    fn get(&self, column: &'static str, key: Vec<u8>) -> Result<Option<Vec<u8>>, Error>;
    fn contains(&self, column: &'static str, key: Vec<u8>) -> Result<bool, Error>;
    fn write_batch(
        &self,
        column: &'static str,
        batch: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> Result<(), Error>;
    fn flush(&self) -> Result<usize, Error>;
}

pub type BackendIterator = dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), Error>>;

pub trait TezedgeDatabaseBackendStoreIterator {
    fn iterator(
        &self,
        column: &'static str,
        mode: BackendIteratorMode,
    ) -> Result<Box<BackendIterator>, Error>;
    fn prefix_iterator(
        &self,
        column: &'static str,
        key: &Vec<u8>,
        max_key_len: usize,
    ) -> Result<Box<BackendIterator>, Error>;
}
