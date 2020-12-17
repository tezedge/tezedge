// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;

use getset::CopyGetters;
use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::has_encoding;

use crate::non_cached_data;

#[derive(Serialize, Deserialize, CopyGetters, Clone)]
pub struct MetadataMessage {
    #[get_copy = "pub"]
    disable_mempool: bool,
    #[get_copy = "pub"]
    private_node: bool,
}

impl MetadataMessage {
    pub fn new(disable_mempool: bool, private_node: bool) -> Self {
        MetadataMessage {
            disable_mempool,
            private_node,
        }
    }
}

impl fmt::Debug for MetadataMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[disable_mempool: {}, private_node: {:?}]",
            self.disable_mempool, self.private_node
        )
    }
}

non_cached_data!(MetadataMessage);
has_encoding!(MetadataMessage, METADATA_MESSAGE_ENCODING, {
    Encoding::Obj(vec![
        Field::new("disable_mempool", Encoding::Bool),
        Field::new("private_node", Encoding::Bool),
    ])
});
