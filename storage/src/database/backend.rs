use crate::database::error::Error;
use crate::Direction;

pub type BoxedSliceKV = (Box<[u8]>, Box<[u8]>);
pub type BackendIterator<'a> = Box<dyn 'a + Send + Iterator<Item = Result<BoxedSliceKV, Error>>>;

#[derive(Clone)]
pub enum BackendIteratorMode {
    Start,
    End,
    From(Vec<u8>, Direction),
}

pub trait TezedgeDatabaseBackendStore {
    fn put(&self, column: &'static str, key: &[u8], value: &[u8]) -> Result<(), Error>;
    fn delete(&self, column: &'static str, key: &[u8]) -> Result<(), Error>;
    fn merge(&self, column: &'static str, key: &[u8], value: &[u8]) -> Result<(), Error>;
    fn get(&self, column: &'static str, key: &[u8]) -> Result<Option<Vec<u8>>, Error>;
    fn contains(&self, column: &'static str, key: &[u8]) -> Result<bool, Error>;
    fn write_batch(
        &self,
        column: &'static str,
        batch: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> Result<(), Error>;
    fn flush(&self) -> Result<usize, Error>;

    fn find<'a>(
        &'a self,
        column: &'static str,
        mode: BackendIteratorMode,
    ) -> Result<BackendIterator<'a>, Error>;
    fn find_by_prefix<'a>(
        &'a self,
        column: &'static str,
        key: &Vec<u8>,
        max_key_len: usize,
    ) -> Result<BackendIterator<'a>, Error>;
}
