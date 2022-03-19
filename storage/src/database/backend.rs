// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::database::error::Error;
use crate::Direction;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::HashMap;
use std::time::Duration;

pub type BoxedSliceKV = (Box<[u8]>, Box<[u8]>);
pub type BackendIterator<'a> = Box<dyn 'a + Send + Iterator<Item = Result<BoxedSliceKV, Error>>>;

#[derive(Clone)]
pub enum BackendIteratorMode {
    Start,
    End,
    From(Vec<u8>, Direction),
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct DBStats {
    pub total_reads: u64,
    #[serde(serialize_with = "to_u128")]
    pub total_read_duration: Duration,
    pub total_writes: u64,
    #[serde(serialize_with = "to_u128")]
    pub total_write_duration: Duration,
    pub total_updates: u64,
    #[serde(serialize_with = "to_u128")]
    pub total_update_duration: Duration,
}

fn to_u128<S>(x: &Duration, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_some(&x.as_micros())
}

#[allow(clippy::ptr_arg)]
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
    fn size(&self) -> HashMap<&'static str, usize>;
    fn sync(&self) -> Result<(), Error>;
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

    fn column_stats(&self) -> HashMap<&'static str, DBStats>;
}
