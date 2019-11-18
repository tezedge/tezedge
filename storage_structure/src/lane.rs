use std::sync::Arc;
use storage::{
    StorageError,
    persistent::{Codec, SchemaError, DatabaseWithSchema, Schema},
};
use serde::{Serialize, Deserialize};
use serde::export::PhantomData;
use rocksdb::DB;
use crate::content::{NodeHeader, ListValue};

type LaneDatabase<T> = dyn DatabaseWithSchema<Lane<T>> + Sync + Send;

/// Lane is an way to traverse the chain.
/// Lane is just an linked list, containing all changes between nodes.
/// There should be multiple lanes, to be able to skip multiple nodes and traverse structure faster.
#[derive(Clone, Debug)]
pub struct Lane<C: ListValue> {
    level: usize,
    db: Arc<DB>,
    _pd: PhantomData<C>,
}

impl<C: ListValue> Schema for Lane<C> {
    const COLUMN_FAMILY_NAME: &'static str = "skip_list_lanes";
    type Key = NodeHeader;
    type Value = C;
}

impl<C: ListValue> Lane<C> {
    pub fn new(level: usize, db: Arc<DB>) -> Self {
        Self {
            level,
            db,
            _pd: PhantomData,
        }
    }

    pub fn lower_lane(self) -> Self {
        Self::new(self.level - 1, self.db)
    }

    pub fn level(&self) -> usize {
        self.level
    }

    pub fn get(&self, index: usize) -> () {
        let container = Self::container(self.db.clone());
    }

    #[inline]
    fn container(db: Arc<LaneDatabase<C>>) -> Arc<LaneDatabase<C>> {
        db
    }
}