// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

// NOTE: made a separate file, the operatins RPC will encapsulate more functionality,
//       for now, it is just one function, that fetches the operations

use failure::bail;

use storage::{BlockStorage, BlockStorageReader};
use storage::persistent::PersistentStorage;

use crate::helpers::{get_block_hash_by_block_id, BlockOperations};
use crate::rpc_actor::RpcCollectedStateRef;

pub(crate) fn get_operations(_chain_id: &str, block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Option<BlockOperations>, failure::Error> {
    let block_storage = BlockStorage::new(persistent_storage);
    let block_hash = get_block_hash_by_block_id(block_id, persistent_storage, state)?;
    let operations = block_storage.get_with_json_data(&block_hash)?.map(|(_, json_data)| json_data.operations_proto_metadata_json().clone());
    match operations {
        Some(op) => Ok(Some(serde_json::from_str(&op)?)),
        None => bail!("No operations found")
    }
}