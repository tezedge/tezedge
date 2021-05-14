use crate::database::error::Error;
use crate::Direction;
use crate::persistent::SchemaError;

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

    fn find(&self, column: &'static str, mode: BackendIteratorMode, limit : Option<usize>, filter : Box<dyn Fn((&[u8],&[u8])) -> Result<bool,SchemaError>>) -> Result<Vec<(Box<[u8]>, Box<[u8]>)>, Error> ;
    fn find_by_prefix(&self, column: &'static str,key: &Vec<u8>, max_key_len: usize,filter : Box<dyn Fn((&[u8],&[u8])) -> Result<bool,SchemaError>> ) -> Result<Vec<(Box<[u8]>, Box<[u8]>)>, Error>;
}