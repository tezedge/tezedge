// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::persistent::codec::Codec;

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
