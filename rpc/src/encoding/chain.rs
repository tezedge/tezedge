// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;

use crate::helpers::{FullBlockInfo, InnerBlockHeader};

use super::base_types::*;

// { protocol: ProtocolHash,
//   chain_id: ChainHash,
//   hash: BlockHash,
//   header: BlockHeader,
//   metadata: BlockMetadata,
//   operations: Vec<BlockOperations> }
#[derive(Serialize, Debug)]
pub struct BlockInfo {
    protocol: Option<UniString>,
    chain_id: Option<UniString>,
    hash: Option<UniString>,
    header: InnerBlockHeader,
    metadata: HashMap<String, Value>,
    operations: Vec<Vec<HashMap<String, Value>>>,
}

impl From<FullBlockInfo> for BlockInfo {
    fn from(val: FullBlockInfo) -> Self {
        Self {
            protocol: None,
            chain_id: Some(val.chain_id.into()),
            hash: Some(val.hash.into()),
            header: val.header,
            operations: val.operations,
            metadata: val.metadata,
        }
    }
}