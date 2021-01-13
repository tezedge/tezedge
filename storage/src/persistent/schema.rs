// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use rocksdb::{Cache, ColumnFamilyDescriptor};

use crate::persistent::codec::Codec;
use crate::persistent::default_table_options;

/// This trait extends basic column family by introducing Codec types safety and enforcement
pub trait KeyValueSchema {
    type Key: Codec;
    type Value: Codec;

    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        ColumnFamilyDescriptor::new(Self::name(), default_table_options(cache))
    }

    fn name() -> &'static str;
}

pub struct CommitLogDescriptor {
    name: String,
}

impl CommitLogDescriptor {
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}

pub trait CommitLogSchema {
    // TODO: split value to `ValueIn` and `ValueOut` - we will start to use references in `ValueIn` but that will introduce
    //       lifetime bound which is not currently supported for associated types. Unless we want to all lifetime
    //       to the `CommitLogSchema`.
    type Value: Codec;

    fn descriptor() -> CommitLogDescriptor {
        CommitLogDescriptor {
            name: Self::name().into(),
        }
    }

    fn name() -> &'static str;
}
