// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

// Clippy complains about some types returned to Ocaml
#![allow(clippy::type_complexity)]

//! Functions exposed to be called from OCaml

use core::borrow::Borrow;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use std::{cell::RefCell, convert::TryFrom, rc::Rc, sync::Arc};

use ocaml_interop::*;

use crypto::hash::ContextHash;

use crate::{
    from_ocaml::OCamlQueryKind,
    initializer::initialize_tezedge_index,
    snapshot, timings,
    working_tree::{
        storage::DirectoryId,
        working_tree::{FoldDepth, FoldOrder, TreeWalker, WorkingTree},
        DirEntryKind,
    },
    ContextKeyValueStore, IndexApi, PatchContextFunction, ProtocolContextApi, ShellContextApi,
    TezedgeContext, TezedgeIndex,
};
use tezos_context_api::TezosContextTezEdgeStorageConfiguration;
use tezos_conv::{
    OCamlBlockHash, OCamlContextHash, OCamlOperationHash, OCamlProtocolHash,
    OCamlTezosContextTezEdgeStorageConfiguration,
};

// TODO: instead of converting errors into strings, it may be useful to pass
// them around using custom pointers so that they can be recovered later.
// OCaml code will not do anything with the errors, just raise an exception,
// but once we catch it on Rust, having the original error value may be useful.

// TODO: see if some of the cloning can be avoided.

enum TreeKind {
    Tree,
    Value,
}

impl_to_ocaml_polymorphic_variant! {
    TreeKind {
        TreeKind::Tree,
        TreeKind::Value
    }
}

type WorkingTreeFFI = WorkingTree;
#[derive(Clone)]
struct TreeWalkerFFI(Rc<RefCell<TreeWalker>>);
#[derive(Clone)]
struct TezedgeIndexFFI(RefCell<TezedgeIndex>);
#[derive(Clone)]
struct TezedgeContextFFI(RefCell<TezedgeContext>);

impl TreeWalkerFFI {
    fn new(walker: TreeWalker) -> Self {
        Self(Rc::new(RefCell::new(walker)))
    }
}

impl From<TreeWalker> for TreeWalkerFFI {
    fn from(walker: TreeWalker) -> Self {
        Self::new(walker)
    }
}

impl TezedgeIndexFFI {
    fn new(index: TezedgeIndex) -> Self {
        Self(RefCell::new(index))
    }
}

impl From<TezedgeIndex> for TezedgeIndexFFI {
    fn from(index: TezedgeIndex) -> Self {
        Self::new(index)
    }
}

impl TezedgeContextFFI {
    fn new(index: TezedgeContext) -> Self {
        Self(RefCell::new(index))
    }
}

impl From<TezedgeContext> for TezedgeContextFFI {
    fn from(index: TezedgeContext) -> Self {
        Self::new(index)
    }
}

/// Array where we store the context keys from ocaml.
///
/// This avoid the cost to allocate a Vec<_> on each ffi call.
#[allow(clippy::large_enum_variant)]
enum KeysArray<'a> {
    Inlined {
        length: usize,
        array: [&'a str; KEYS_ARRAY_LENGTH],
    },
    Heap(Vec<&'a str>),
}

const KEYS_ARRAY_LENGTH: usize = 32;

impl<'a> KeysArray<'a> {
    fn new() -> Self {
        Self::Inlined {
            length: 0,
            array: [""; KEYS_ARRAY_LENGTH],
        }
    }

    fn push(&mut self, str_ref: &'a str) {
        match self {
            KeysArray::Inlined { length, array } => {
                if *length < KEYS_ARRAY_LENGTH {
                    array[*length] = str_ref;
                    *length += 1;
                } else {
                    let mut vec = Vec::with_capacity(KEYS_ARRAY_LENGTH * 2);
                    vec.extend_from_slice(&array[..]);
                    vec.push(str_ref);
                    *self = Self::Heap(vec);
                }
            }
            KeysArray::Heap(heap) => {
                heap.push(str_ref);
            }
        }
    }
}

impl<'a> std::ops::Deref for KeysArray<'a> {
    type Target = [&'a str];

    fn deref(&self) -> &Self::Target {
        match self {
            KeysArray::Inlined { length, array } => &array[..*length],
            KeysArray::Heap(heap) => heap,
        }
    }
}

fn make_key<'a>(rt: &'a OCamlRuntime, key: OCamlRef<OCamlList<String>>) -> KeysArray<'a> {
    let mut key = rt.get(key);

    let mut vector: KeysArray = KeysArray::new();

    while let Some((head, tail)) = key.uncons() {
        vector.push(unsafe { head.as_str_unchecked() });
        key = tail;
    }

    vector
}

// TODO: move this static and its accessors to a better place
lazy_static! {
    pub static ref TEZEDGE_CONTEXT_REPOSITORY: RwLock<Option<Arc<RwLock<ContextKeyValueStore>>>> =
        RwLock::new(None);
}

#[derive(Debug)]
pub enum TezedgeIndexError {
    LockPoisonError,
}

// Set the repository so that it can be accessed by the context IPC
fn set_context_index(index: &TezedgeIndex) -> Result<(), TezedgeIndexError> {
    TEZEDGE_CONTEXT_REPOSITORY
        .write()
        .replace(Arc::clone(&index.repository));
    Ok(())
}

// Obtain the context index (for context IPC access)
pub fn get_context_index() -> Result<Option<TezedgeIndex>, TezedgeIndexError> {
    Ok(TEZEDGE_CONTEXT_REPOSITORY
        .read()
        .as_ref()
        .map(|repository| TezedgeIndex::new(Arc::clone(repository), None)))
}

ocaml_export! {
    // Index API

    fn tezedge_index_init(
        rt,
        configuration: OCamlRef<OCamlTezosContextTezEdgeStorageConfiguration>,
        patch_context: OCamlRef<Option<PatchContextFunction>>,
    ) -> OCaml<Result<DynBox<TezedgeIndexFFI>, String>> {
        let patch_context = rt.get(patch_context).to_option().map(BoxRoot::new);
        let configuration: TezosContextTezEdgeStorageConfiguration = configuration.to_rust(rt);
        let result = initialize_tezedge_index(&configuration, patch_context)
            .map_err(|err| format!("{:?}", err));

        if let Ok(index) = &result {
            if let Err(err) = set_context_index(index) {
                Err::<BoxRoot<DynBox<TezedgeIndexFFI>>, String>(format!("{:?}", err)).to_ocaml(rt)
            } else {
                result.map(|index| OCaml::box_value(rt, index.into()).root()).to_ocaml(rt)
            }
        } else {
            result.map(|index| OCaml::box_value(rt, index.into()).root()).to_ocaml(rt)
        }
    }

    fn tezedge_index_latest_context_hashes(
        rt,
        index: OCamlRef<DynBox<TezedgeIndexFFI>>,
        count: OCamlRef<OCamlInt>,
    ) -> OCaml<Result<OCamlList<OCamlContextHash>, String>> {
        let ocaml_index = rt.get(index);
        let count: i64 = count.to_rust(rt);
        let index: &TezedgeIndexFFI = ocaml_index.borrow();
        let index = index.0.borrow().clone();

        let result = index.latest_context_hashes(count)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    fn tezedge_index_close(
        rt,
        _index: OCamlRef<DynBox<TezedgeIndexFFI>>,
    ) {
        OCaml::unit()
    }

    fn tezedge_index_patch_context_get(
        rt,
        index: OCamlRef<DynBox<TezedgeIndexFFI>>,
    ) -> OCaml<Option<PatchContextFunction>> {
        let ocaml_index = rt.get(index);
        let index: &TezedgeIndexFFI = ocaml_index.borrow();
        let index = index.0.borrow().clone();
        index.patch_context.to_ocaml(rt)
    }

    // OCaml = val exists : index -> Context_hash.t -> bool Lwt.t
    fn tezedge_index_exists(
        rt,
        index: OCamlRef<DynBox<TezedgeIndexFFI>>,
        context_hash: OCamlRef<OCamlContextHash>,
    ) -> OCaml<Result<bool, String>> {
        let ocaml_index = rt.get(index);
        let index: &TezedgeIndexFFI = ocaml_index.borrow();
        let index = index.0.borrow().clone();
        let context_hash: ContextHash = context_hash.to_rust(rt);

        let result = index.exists(&context_hash)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    // OCaml = val checkout : index -> Context_hash.t -> context option Lwt.t
    fn tezedge_index_checkout(
        rt,
        index: OCamlRef<DynBox<TezedgeIndexFFI>>,
        context_hash: OCamlRef<OCamlContextHash>,
    ) -> OCaml<Result<Option<DynBox<TezedgeContextFFI>>, String>> {
        let ocaml_index = rt.get(index);
        let index: &TezedgeIndexFFI = ocaml_index.borrow();
        let index = index.0.borrow().clone();
        let context_hash: ContextHash = context_hash.to_rust(rt);

        let result = index.checkout(&context_hash)
            .map_err(|err| format!("{:?}", err))
            .map(|opt| opt.map(TezedgeContextFFI::new));

        result.to_ocaml(rt)
    }

    fn tezedge_index_block_applied(
        rt,
        index: OCamlRef<DynBox<TezedgeIndexFFI>>,
        context_hash: OCamlRef<Option<OCamlContextHash>>,
        extra_data: OCamlRef<(OCamlInt32, Option<OCamlInt>)>,
    ) -> OCaml<Result<(), String>> {
        let ocaml_index = rt.get(index);
        let index: &TezedgeIndexFFI = ocaml_index.borrow();
        let index = index.0.borrow().clone();
        let (block_level, _cycle_position): (i32, Option<i64>) = extra_data.to_rust(rt);
        let context_hash: Option<ContextHash> = context_hash.to_rust(rt);

        // We call `IndexApi::block_applied` only when `context_hash` is `Some(_)`:
        // For a single commit, `tezedge_index_block_applied` is called twice from OCaml
        // Once before the commit, and one more time after the commit.
        // Before the commit, it is called with a `None` `context_hash`, and after
        // the commit, it is called with a `Some(_)` `context_hash`
        let result = match context_hash {
            Some(ref context_hash) => {
                let block_level = block_level as u32;
                index.block_applied(block_level, context_hash)
                     .map_err(|err| format!("BlockApplied: {:?}", err))
            },
            _ => Ok(())
        };

        result.to_ocaml(rt)
    }

    // Context API

    // OCaml = val commit : time:Time.Protocol.t -> ?message:string -> context -> Context_hash.t Lwt.t
    fn tezedge_context_commit(
        rt,
        date: OCamlRef<OCamlInt64>,
        message: OCamlRef<String>,
        author: OCamlRef<String>,
        context: OCamlRef<DynBox<TezedgeContextFFI>>,
    ) -> OCaml<Result<OCamlContextHash, String>> {
        let ocaml_context = rt.get(context);
        let context: &TezedgeContextFFI = ocaml_context.borrow();
        let context = context.0.borrow().clone();
        let message = message.to_rust(rt);
        let date = date.to_rust(rt);
        let author = author.to_rust(rt);

        // TODO: return commit value instead of hash?
        let result = context.commit(author, message, date)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    // OCaml = val hash : time:Time.Protocol.t -> ?message:string -> context -> Context_hash.t
    fn tezedge_context_hash(
        rt,
        date: OCamlRef<OCamlInt64>,
        message: OCamlRef<String>,
        author: OCamlRef<String>,
        context: OCamlRef<DynBox<TezedgeContextFFI>>,
    ) -> OCaml<Result<OCamlContextHash, String>> {
        let ocaml_context = rt.get(context);
        let context: &TezedgeContextFFI = ocaml_context.borrow();
        let context = context.0.borrow().clone();
        let message = message.to_rust(rt);
        let date = date.to_rust(rt);
        let author = author.to_rust(rt);

        let result = context.hash(author, message, date)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    // OCaml = val mem : context -> key -> bool Lwt.t
    fn tezedge_context_mem(
        rt,
        context: OCamlRef<DynBox<TezedgeContextFFI>>,
        key: OCamlRef<OCamlList<String>>,
    ) -> OCaml<Result<bool, String>> {
        let ocaml_context = rt.get(context);
        let context: &TezedgeContextFFI = ocaml_context.borrow();
        let context = context.0.borrow().clone();
        let key = make_key(rt, key);

        let result = context.mem(&key)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    fn tezedge_context_empty(
        rt,
        index: OCamlRef<DynBox<TezedgeIndexFFI>>,
    ) -> OCaml<DynBox<TezedgeContextFFI>> {
        let ocaml_index = rt.get(index);
        let index: &TezedgeIndexFFI = ocaml_index.borrow();
        let index = index.0.borrow().clone();
        let empty_context = TezedgeContext::new(index, None, None);

        OCaml::box_value(rt, empty_context.into())
    }

    // OCaml = val mem_tree : context -> key -> bool Lwt.t
    fn tezedge_context_mem_tree(
        rt,
        context: OCamlRef<DynBox<TezedgeContextFFI>>,
        key: OCamlRef<OCamlList<String>>,
    ) -> OCaml<bool> {
        let ocaml_context = rt.get(context);
        let context: &TezedgeContextFFI = ocaml_context.borrow();
        let context = context.0.borrow().clone();
        let key = make_key(rt, key);

        let result = context.mem_tree(&key);

        result.to_ocaml(rt)
    }

    // OCaml = val find : context -> key -> value option Lwt.t
    fn tezedge_context_find(
        rt,
        context: OCamlRef<DynBox<TezedgeContextFFI>>,
        key: OCamlRef<OCamlList<String>>,
    ) -> OCaml<Result<Option<OCamlBytes>, String>> {
        let ocaml_context = rt.get(context);
        let context: &TezedgeContextFFI = ocaml_context.borrow();
        let context = context.0.borrow().clone();
        let key = make_key(rt, key);

        let result = context.find(&key)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    // OCaml = val find_tree : context -> key -> tree option Lwt.t
    fn tezedge_context_find_tree(
        rt,
        context: OCamlRef<DynBox<TezedgeContextFFI>>,
        key: OCamlRef<OCamlList<String>>,
    ) -> OCaml<Result<Option<DynBox<WorkingTreeFFI>>, String>> {
        let ocaml_context = rt.get(context);
        let context: &TezedgeContextFFI = ocaml_context.borrow();
        let context = context.0.borrow().clone();
        let key = make_key(rt, key);

        let result = context.find_tree(&key)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    // OCaml = val add : context -> key -> value -> t Lwt.t
    fn tezedge_context_add(
        rt,
        context: OCamlRef<DynBox<TezedgeContextFFI>>,
        key: OCamlRef<OCamlList<String>>,
        value: OCamlRef<OCamlBytes>,
    ) -> OCaml<Result<DynBox<TezedgeContextFFI>, String>> {
        let ocaml_context = rt.get(context);
        let context: &TezedgeContextFFI = ocaml_context.borrow();
        let context = context.0.borrow().clone();
        let key = make_key(rt, key);

        let value_ref = rt.get(value);
        let value = value_ref.as_bytes();

        let result = context.add(&key, value)
            .map_err(|err| format!("{:?}", err))
            .map(TezedgeContextFFI::new);

        result.to_ocaml(rt)
    }

    // OCaml = val add_tree : context -> key -> tree -> t Lwt.t
    fn tezedge_context_add_tree(
        rt,
        context: OCamlRef<DynBox<TezedgeContextFFI>>,
        key: OCamlRef<OCamlList<String>>,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
    ) -> OCaml<Result<DynBox<TezedgeContextFFI>, String>> {
        let ocaml_context = rt.get(context);
        let context: &TezedgeContextFFI = ocaml_context.borrow();
        let context = context.0.borrow().clone();
        let key = make_key(rt, key);
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();

        let result =  context.add_tree(&key, tree)
            .map_err(|err| format!("{:?}", err))
            .map(TezedgeContextFFI::new);

         result.to_ocaml(rt)
    }

    // OCaml = val remove_ : context -> key -> t Lwt.t
    fn tezedge_context_remove(
        rt,
        context: OCamlRef<DynBox<TezedgeContextFFI>>,
        key: OCamlRef<OCamlList<String>>,
    ) -> OCaml<Result<DynBox<TezedgeContextFFI>, String>> {
        let ocaml_context = rt.get(context);
        let context: &TezedgeContextFFI = ocaml_context.borrow();
        let context = context.0.borrow().clone();
        let key = make_key(rt, key);

        let result = context.delete(&key)
            .map_err(|err| format!("{:?}", err))
            .map(TezedgeContextFFI::new);

        result.to_ocaml(rt)
    }

    // OCaml = val list : context -> ?offset:int -> ?length:int -> key -> (string * tree) list Lwt.t
    fn tezedge_context_list(
        rt,
        context: OCamlRef<DynBox<TezedgeContextFFI>>,
        offset: OCamlRef<Option<OCamlInt>>,
        length: OCamlRef<Option<OCamlInt>>,
        key: OCamlRef<OCamlList<String>>,
    ) -> OCaml<Result<OCamlList<(String, DynBox<WorkingTreeFFI>)>, String>> {
        let ocaml_context = rt.get(context);
        let context: &TezedgeContextFFI = ocaml_context.borrow();
        let context = context.0.borrow().clone();
        let offset: Option<i64> = offset.to_rust(rt);
        let offset = offset.map(|n| n as usize);
        let length: Option<i64> = length.to_rust(rt);
        let length = length.map(|n| n as usize);
        let key = make_key(rt, key);

        let result = context
            .list(offset, length, &key)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    fn tezedge_context_get_tree(
        rt,
        context: OCamlRef<DynBox<TezedgeContextFFI>>,
    ) -> OCaml<DynBox<WorkingTreeFFI>> {
        let ocaml_context = rt.get(context);
        let context: &TezedgeContextFFI = ocaml_context.borrow();
        let tree = Rc::clone(&context.0.borrow().tree);

        tree.to_ocaml(rt)
    }

    fn tezedge_context_set_tree(
        rt,
        context: OCamlRef<DynBox<TezedgeContextFFI>>,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
    ) -> OCaml<DynBox<TezedgeContextFFI>> {
        let ocaml_context = rt.get(context);
        let context: &TezedgeContextFFI = ocaml_context.borrow();
        let mut context = context.0.borrow().clone();
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();

        context.tree = Rc::new(tree.clone());

        let result = TezedgeContextFFI::new(context);

        result.to_ocaml(rt)
    }

    // Tree API

    // OCaml = val hash : tree -> Context_hash.t
    fn tezedge_tree_hash(
        rt,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
    ) -> OCaml<Result<OCamlContextHash, String>> {
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();

        let result = tree
            .hash()
            .map_err(|err| format!("{:?}", err))
            .map(|h| {
                ContextHash::try_from(&h[..]).map_err(|err| format!("{:?}", err))
            })
            .unwrap_or_else(Err);

        result.to_ocaml(rt)
    }

    // OCaml = val mem : tree -> key -> bool Lwt.t
    fn tezedge_tree_mem(
        rt,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
        key: OCamlRef<OCamlList<String>>,
    ) -> OCaml<Result<bool, String>> {
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();
        let key = make_key(rt, key);

        let result = tree.mem(&key)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    // OCaml = val empty : context -> tree
    fn tezedge_tree_empty(
        rt,
        context: OCamlRef<DynBox<TezedgeContextFFI>>,
    ) -> OCaml<DynBox<WorkingTreeFFI>> {
        let ocaml_context = rt.get(context);
        let context: &TezedgeContextFFI = ocaml_context.borrow();
        let context = context.0.borrow().clone();
        let empty_tree = WorkingTree::new_with_directory(context.index, DirectoryId::empty());

        empty_tree.to_ocaml(rt)
    }

    // OCaml = val to_value : tree -> value option Lwt.t
    fn tezedge_tree_to_value(
        rt,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
    ) -> OCaml<Option<OCamlBytes>> {
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();

        let result = tree.get_value();

        result.to_ocaml(rt)
    }

    // OCaml = val of_value : context -> value -> tree Lwt.t
    fn tezedge_tree_of_value(
        rt,
        context: OCamlRef<DynBox<TezedgeContextFFI>>,
        value: OCamlRef<OCamlBytes>
    ) -> OCaml<Result<DynBox<WorkingTreeFFI>, String>> {
        let ocaml_context = rt.get(context);
        let context: &TezedgeContextFFI = ocaml_context.borrow();
        let context = context.0.borrow().clone();
        let value = rt.get(value);

        let mut storage = context.index.storage.borrow_mut();

        let result = match storage.add_blob_by_ref(value.as_bytes()) {
            Ok(blob_id) => {
                std::mem::drop(storage);
                Ok(WorkingTree::new_with_value(context.index, blob_id))
            },
            Err(err) => Err(format!("{:?}", err))
        };

        result.to_ocaml(rt)
    }

    // OCaml = val is_empty : tree -> bool
    fn tezedge_tree_is_empty(
        rt,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
    ) -> OCaml<bool> {
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();

        let result = tree.is_empty();

        result.to_ocaml(rt)
    }

    // OCaml = val equal : tree -> tree -> bool
    fn tezedge_tree_equal(
        rt,
        tree1: OCamlRef<DynBox<WorkingTreeFFI>>,
        tree2: OCamlRef<DynBox<WorkingTreeFFI>>,
    ) -> OCaml<Result<bool, String>> {
        let ocaml_tree1 = rt.get(tree1);
        let tree1: &WorkingTreeFFI = ocaml_tree1.borrow();
        let ocaml_tree2 = rt.get(tree2);
        let tree2: &WorkingTreeFFI = ocaml_tree2.borrow();
        let result = tree1.equal(tree2)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    // OCaml = val kind : tree -> [`Value | `Tree]
    fn tezedge_tree_kind(
        rt,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
    ) -> OCaml<TreeKind> {
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();

        let result = match tree.kind() {
            DirEntryKind::Blob => TreeKind::Value,
            DirEntryKind::Directory => TreeKind::Tree,
        };

        result.to_ocaml(rt)
    }

    // OCaml = val mem_tree : tree -> key -> bool Lwt.t
    fn tezedge_tree_mem_tree(
        rt,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
        key: OCamlRef<OCamlList<String>>,
    ) -> OCaml<bool> {
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();
        let key = make_key(rt, key);

        let result = tree.mem_tree(&key);

        result.to_ocaml(rt)
    }

    // OCaml = val find : tree -> key -> value option Lwt.t
    fn tezedge_tree_find(
        rt,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
        key: OCamlRef<OCamlList<String>>,
    ) -> OCaml<Result<Option<OCamlBytes>, String>> {
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();
        let key = make_key(rt, key);

        let result = tree.find(&key)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    // OCaml = val find_tree : tree -> key -> tree option Lwt.t
    fn tezedge_tree_find_tree(
        rt,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
        key: OCamlRef<OCamlList<String>>,
    ) -> OCaml<Result<Option<DynBox<WorkingTreeFFI>>, String>> {
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();
        let key = make_key(rt, key);

        let result = tree.find_tree(&key)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    // OCaml = val add : tree -> key -> value -> t Lwt.t
    fn tezedge_tree_add(
        rt,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
        key: OCamlRef<OCamlList<String>>,
        value: OCamlRef<OCamlBytes>,
    ) -> OCaml<Result<DynBox<WorkingTreeFFI>, String>> {
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();
        let key = make_key(rt, key);

        let value_ref = rt.get(value);
        let value = value_ref.as_bytes();

        let result =  tree.add(&key, value)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    // OCaml = val add_tree : tree -> key -> tree -> t Lwt.t
    fn tezedge_tree_add_tree(
        rt,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
        key: OCamlRef<OCamlList<String>>,
        new_tree: OCamlRef<DynBox<WorkingTreeFFI>>,
    ) -> OCaml<Result<DynBox<WorkingTreeFFI>, String>> {
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();
        let key = make_key(rt, key);
        let ocaml_new_tree = rt.get(new_tree);
        let new_tree: &WorkingTreeFFI = ocaml_new_tree.borrow();

        let result =  tree.add_tree(&key, new_tree)
            .map_err(|err| format!("{:?}", err));

         result.to_ocaml(rt)
    }

    // OCaml = val remove : tree -> key -> t Lwt.t
    fn tezedge_tree_remove(
        rt,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
        key: OCamlRef<OCamlList<String>>,
    ) -> OCaml<Result<DynBox<WorkingTreeFFI>, String>> {
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();
        let key = make_key(rt, key);

        let result = tree.delete(&key)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    // OCaml = val list : tree -> ?offset:int -> ?length:int -> key -> (string * tree) list Lwt.t
    fn tezedge_tree_list(
        rt,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
        offset: OCamlRef<Option<OCamlInt>>,
        length: OCamlRef<Option<OCamlInt>>,
        key: OCamlRef<OCamlList<String>>,
    ) -> OCaml<Result<OCamlList<(String, DynBox<WorkingTreeFFI>)>, String>> {
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();
        let offset: Option<i64> = offset.to_rust(rt);
        let offset = offset.map(|n| n as usize);
        let length: Option<i64> = length.to_rust(rt);
        let length = length.map(|n| n as usize);
        let key = make_key(rt, key);

        let result = tree
            .list(offset, length, &key)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    // OCaml =
    //  val fold :
    //    ?depth:[`Eq of int | `Le of int | `Lt of int | `Ge of int | `Gt of int] ->
    //    t ->
    //    key ->
    //    init:'a ->
    //    f:(key -> tree -> 'a -> 'a Lwt.t) ->
    //    'a Lwt.t
    fn tezedge_tree_walker_make(
        rt,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
        depth: OCamlRef<Option<FoldDepth>>,
        key: OCamlRef<OCamlList<String>>,
        order: OCamlRef<FoldOrder>,
    ) -> OCaml<Result<DynBox<TreeWalkerFFI>, String>> {
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();
        let key = make_key(rt, key);
        let depth: Option<FoldDepth> = depth.to_rust(rt);
        let order: FoldOrder = order.to_rust(rt);
        let result = tree.fold_iter(depth, &key, order)
            .map_err(|err| format!("{:?}", err))
            .map(TreeWalkerFFI::new);

        result.to_ocaml(rt)
    }

    fn tezedge_tree_walker_next(
        rt,
        walker: OCamlRef<DynBox<TreeWalkerFFI>>,
    ) -> OCaml<Option<(OCamlList<String>, DynBox<WorkingTreeFFI>)>> {
        let ocaml_walker = rt.get(walker);
        let walker: &TreeWalkerFFI = ocaml_walker.borrow();
        let result = walker.0.borrow_mut().next();

        result.to_ocaml(rt)
    }

    // Timings

    fn tezedge_timing_set_block(
        rt,
        block_hash: OCamlRef<Option<OCamlBlockHash>>,
    ) {
        timings::set_block(rt, block_hash);
        OCaml::unit()
    }

    fn tezedge_timing_set_protocol(
        rt,
        protocol_hash: OCamlRef<OCamlProtocolHash>,
    ) {
        timings::set_protocol(rt, protocol_hash);
        OCaml::unit()
    }

    fn tezedge_timing_checkout(
        rt,
        context_hash: OCamlRef<OCamlContextHash>,
        irmin_time: f64,
        tezedge_time: f64,
    ) {
        timings::checkout(rt, context_hash, irmin_time, tezedge_time);
        OCaml::unit()
    }

    fn tezedge_timing_set_operation(
        rt,
        operation_hash: OCamlRef<Option<OCamlOperationHash>>,
    ) {
        timings::set_operation(rt, operation_hash);
        OCaml::unit()
    }

    fn tezedge_timing_commit(
        rt,
        new_context_hash: OCamlRef<OCamlContextHash>,
        irmin_time: f64,
        tezedge_time: f64,
    ) {
        timings::commit(rt, new_context_hash, irmin_time, tezedge_time);
        OCaml::unit()
    }

    fn tezedge_timing_context_action(
        rt,
        query_kind: OCamlRef<OCamlQueryKind>,
        key: OCamlRef<OCamlList<String>>,
        irmin_time: f64,
        tezedge_time: f64,
    ) {
        timings::context_query(rt, query_kind, key, irmin_time, tezedge_time);
        OCaml::unit()
    }

    fn tezedge_timing_init(
        rt,
        db_path: OCamlRef<String>,
    ) {
        let db_path: String = db_path.to_rust(rt);

        timings::init_timing(db_path);
        OCaml::unit()
    }

    // Snapshots

    fn tezedge_context_dump(
        rt,
        context_path: OCamlRef<String>,
        context_hash: OCamlRef<OCamlContextHash>,
        dump_path: OCamlRef<String>,
    ) -> OCaml<Result<OCamlInt, String>> {
        // TODO: remove unwraps
        let context_path = context_path.to_rust(rt);
        let ctx = snapshot::reload_context_readonly(context_path).unwrap();
        let dump_path: String = dump_path.to_rust(rt);
        let context_hash: ContextHash = context_hash.to_rust(rt);

        let (tree, storage, string_interner, parent_hash, commit) =
            snapshot::read_commit_tree(ctx, &context_hash).unwrap();

        snapshot::create_new_database(
            tree,
            storage,
            string_interner,
            parent_hash,
            commit,
            &dump_path,
            &context_hash,
            log_null,
        )
        .unwrap();

        snapshot::recompute_hashes(&dump_path, &context_hash, log_null)
            .unwrap();

        let result: Result<i32, String> = Ok(0);

        result.to_ocaml(rt)
    }

    fn tezedge_context_restore(
        rt,
        target_path: OCamlRef<String>,
        context_dump_path: OCamlRef<String>,
        _expected_context_hash: OCamlRef<OCamlContextHash>,
        _nb_context_elements: OCamlRef<OCamlInt>,
    ) {
        let target_path: String = target_path.to_rust(rt);
        let context_dump_path: String = context_dump_path.to_rust(rt);
        println!("restoring into {} -> {}", context_dump_path, target_path);
        std::fs::rename(context_dump_path, target_path).unwrap();

        OCaml::unit()
    }
}

fn log_null(_: &str) {}

unsafe impl ToOCaml<DynBox<WorkingTreeFFI>> for WorkingTreeFFI {
    fn to_ocaml<'gc>(&self, rt: &'gc mut OCamlRuntime) -> OCaml<'gc, DynBox<WorkingTreeFFI>> {
        OCaml::box_value(rt, self.clone())
    }
}

unsafe impl ToOCaml<DynBox<TreeWalkerFFI>> for TreeWalkerFFI {
    fn to_ocaml<'gc>(&self, rt: &'gc mut OCamlRuntime) -> OCaml<'gc, DynBox<TreeWalkerFFI>> {
        OCaml::box_value(rt, self.clone())
    }
}

unsafe impl ToOCaml<DynBox<TezedgeContextFFI>> for TezedgeContextFFI {
    fn to_ocaml<'gc>(&self, rt: &'gc mut OCamlRuntime) -> OCaml<'gc, DynBox<TezedgeContextFFI>> {
        OCaml::box_value(rt, self.clone())
    }
}

unsafe impl ToOCaml<DynBox<TezedgeIndexFFI>> for TezedgeIndexFFI {
    fn to_ocaml<'gc>(&self, rt: &'gc mut OCamlRuntime) -> OCaml<'gc, DynBox<TezedgeIndexFFI>> {
        OCaml::box_value(rt, self.clone())
    }
}

use tezos_sys::{
    initialize_tezedge_context_callbacks, initialize_tezedge_index_callbacks,
    initialize_tezedge_timing_callbacks, initialize_tezedge_tree_callbacks,
};

pub fn initialize_callbacks() {
    unsafe {
        initialize_tezedge_context_callbacks(
            tezedge_context_commit,
            tezedge_context_hash,
            tezedge_context_find_tree,
            tezedge_context_add_tree,
            tezedge_context_remove,
            tezedge_context_add,
            tezedge_context_find,
            tezedge_context_mem_tree,
            tezedge_context_mem,
            tezedge_context_list,
            tezedge_context_get_tree,
            tezedge_context_set_tree,
            tezedge_context_empty,
            tezedge_context_dump,
            tezedge_context_restore,
        );
        initialize_tezedge_tree_callbacks(
            tezedge_tree_hash,
            tezedge_tree_find_tree,
            tezedge_tree_add_tree,
            tezedge_tree_remove,
            tezedge_tree_add,
            tezedge_tree_find,
            tezedge_tree_mem_tree,
            tezedge_tree_mem,
            tezedge_tree_list,
            tezedge_tree_walker_make,
            tezedge_tree_walker_next,
            tezedge_tree_empty,
            tezedge_tree_to_value,
            tezedge_tree_of_value,
            tezedge_tree_is_empty,
            tezedge_tree_equal,
            tezedge_tree_kind,
        );
        initialize_tezedge_index_callbacks(
            tezedge_index_patch_context_get,
            tezedge_index_checkout,
            tezedge_index_exists,
            tezedge_index_close,
            tezedge_index_block_applied,
            tezedge_index_init,
            tezedge_index_latest_context_hashes,
        );
        initialize_tezedge_timing_callbacks(
            tezedge_timing_set_block,
            tezedge_timing_set_protocol,
            tezedge_timing_checkout,
            tezedge_timing_set_operation,
            tezedge_timing_commit,
            tezedge_timing_context_action,
            tezedge_timing_init,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keys_array() {
        let mut s = KeysArray::new();

        for i in 0..KEYS_ARRAY_LENGTH + 10 {
            s.push("a");
            let bytes = &*s;

            assert_eq!(bytes.len(), i + 1);
        }
    }
}
