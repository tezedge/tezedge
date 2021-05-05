// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cell::RefCell;
use std::{path::PathBuf, rc::Rc};

use ocaml_interop::{BoxRoot, OCaml};

use crate::gc::mark_move_gced::MarkMoveGCed;
use crate::kv_store::{
    btree_map::BTreeMapBackend, in_memory_backend::InMemoryBackend, sled_backend::SledBackend,
};
use crate::{PatchContextFunction, TezedgeContext, TezedgeIndex};

// TODO: should this be here?
const PRESERVE_CYCLE_COUNT: usize = 7;

#[derive(Debug, Clone)]
pub enum ContextKvStoreConfiguration {
    Sled { path: PathBuf },
    InMem,
    BTreeMap,
    InMemGC,
}

// TODO: are errors not possible here? recheck that
pub fn initialize_tezedge_index(
    context_kv_store: &ContextKvStoreConfiguration,
    patch_context: BoxRoot<Option<PatchContextFunction>>,
) -> TezedgeIndex {
    TezedgeIndex::new(
        match context_kv_store {
            ContextKvStoreConfiguration::Sled { path } => {
                let sled = sled::Config::new()
                    .path(path)
                    .open()
                    .expect("Failed to create/initialize Sled database (db_context)");
                Rc::new(RefCell::new(SledBackend::new(sled)))
            }
            ContextKvStoreConfiguration::InMem => Rc::new(RefCell::new(InMemoryBackend::new())),
            ContextKvStoreConfiguration::BTreeMap => Rc::new(RefCell::new(BTreeMapBackend::new())),
            ContextKvStoreConfiguration::InMemGC => {
                Rc::new(RefCell::new(MarkMoveGCed::<InMemoryBackend>::new(
                    PRESERVE_CYCLE_COUNT,
                )))
            }
        },
        patch_context,
    )
}

pub fn initialize_tezedge_context(
    context_kv_store: &ContextKvStoreConfiguration,
) -> Result<TezedgeContext, failure::Error> {
    let patch_context = OCaml::none().root();
    let index = initialize_tezedge_index(context_kv_store, patch_context);
    Ok(TezedgeContext::new(index, None, None))
}
