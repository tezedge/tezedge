// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::{Arc, RwLock};

use ocaml_interop::BoxRoot;
pub use tezos_api::ffi::ContextKvStoreConfiguration;
use tezos_api::ffi::TezosContextTezEdgeStorageConfiguration;

use crate::gc::mark_move_gced::MarkMoveGCed;
use crate::kv_store::readonly_ipc::ReadonlyIpcBackend;
use crate::kv_store::{btree_map::BTreeMapBackend, in_memory_backend::InMemoryBackend};
use crate::{PatchContextFunction, TezedgeContext, TezedgeIndex};

// TODO: should this be here?
const PRESERVE_CYCLE_COUNT: usize = 7;

// TODO: are errors not possible here? recheck that
pub fn initialize_tezedge_index(
    configuration: &TezosContextTezEdgeStorageConfiguration,
    patch_context: Option<BoxRoot<PatchContextFunction>>,
) -> TezedgeIndex {
    TezedgeIndex::new(
        match configuration.backend {
            ContextKvStoreConfiguration::ReadOnlyIpc => {
                // TODO - TE-261: remove expect
                // TODO - TE-261: client connection can fail
                Arc::new(RwLock::new(ReadonlyIpcBackend::connect(
                    configuration
                        .ipc_socket_path
                        .clone()
                        .expect("Expected IPC socket path for the readonly backend"),
                )))
            }
            ContextKvStoreConfiguration::InMem => Arc::new(RwLock::new(InMemoryBackend::new())),
            ContextKvStoreConfiguration::BTreeMap => Arc::new(RwLock::new(BTreeMapBackend::new())),
            ContextKvStoreConfiguration::InMemGC => {
                Arc::new(RwLock::new(MarkMoveGCed::<InMemoryBackend>::new(
                    PRESERVE_CYCLE_COUNT,
                )))
            }
        },
        patch_context,
    )
}

pub fn initialize_tezedge_context(
    configuration: &TezosContextTezEdgeStorageConfiguration,
) -> Result<TezedgeContext, failure::Error> {
    let index = initialize_tezedge_index(configuration, None);
    Ok(TezedgeContext::new(index, None, None))
}
