// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use rocksdb::{ColumnFamilyDescriptor, Options};

use crate::persistent::codec::Codec;

/// This trait extends basic column family by introducing Codec types safety and enforcement
pub trait KeyValueSchema {
    type Key: Codec;
    type Value: Codec;

    fn descriptor() -> ColumnFamilyDescriptor {
        ColumnFamilyDescriptor::new(Self::name(), Options::default())
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
    type Value: Codec;

    fn descriptor() -> CommitLogDescriptor {
        CommitLogDescriptor {
            name: Self::name().into()
        }
    }

    fn name() -> &'static str;
}
