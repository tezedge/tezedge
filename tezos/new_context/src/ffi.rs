// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Functions exposed to be called from OCaml

// TODO: extend init function

use core::borrow::Borrow;
use std::{cell::RefCell, convert::TryFrom};

use ocaml_interop::*;

use crypto::hash::ContextHash;

use crate::{
    initializer::initialize_tezedge_index,
    initializer::ContextKvStoreConfiguration,
    timings,
    working_tree::{working_tree::WorkingTree, NodeKind},
    ContextKey, ContextValue, IndexApi, PatchContextFunction, ProtocolContextApi, ShellContextApi,
    TezedgeContext, TezedgeIndex,
};
use tezos_api::ocaml_conv::{OCamlBlockHash, OCamlContextHash, OCamlOperationHash};

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
struct TezedgeIndexFFI(RefCell<TezedgeIndex>);
#[derive(Clone)]
struct TezedgeContextFFI(RefCell<TezedgeContext>);

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

ocaml_export! {
    // Index API

    fn tezedge_index_init(
        rt,
        patch_context: OCamlRef<Option<PatchContextFunction>>,
    ) -> OCaml<DynBox<TezedgeIndexFFI>> {
        let index = initialize_tezedge_index(&ContextKvStoreConfiguration::InMem, BoxRoot::new(rt.get(patch_context)));
        OCaml::box_value(rt, index.into())
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
        index.patch_context.get(rt)
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
        let key: ContextKey = key.to_rust(rt);

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
        let empty_context = TezedgeContext::new(index.clone(), None, None);

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
        let key: ContextKey = key.to_rust(rt);

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
        let key: ContextKey = key.to_rust(rt);

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
        let key: ContextKey = key.to_rust(rt);

        let result = context.find_tree(&key)
            .map_err(|err| format!("{:?}", err));
            //.map(|tree_opt| tree_opt.map(|tree| OCamlToRustPointer::alloc_custom(rt, Rc::new(tree))));

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
        let key: ContextKey = key.to_rust(rt);
        let value: ContextValue = value.to_rust(rt);

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
        let key: ContextKey = key.to_rust(rt);
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
        let key: ContextKey = key.to_rust(rt);

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
        let key: ContextKey = key.to_rust(rt);

        // TODO: don't clone the string, implement `ToOCaml` trait for `Rc<_>`
        let result = context
            .list(offset, length, &key)
            .map_err(|err| format!("{:?}", err))
            .map(|v| {
                v.into_iter()
                    .map(|(s, tree)| ((*s).clone(), tree))
                    .collect::<Vec<_>>()
            });

        result.to_ocaml(rt)
    }

    // TODO: fold
    //  (** [fold ?depth t root ~init ~f] recursively folds over the trees
    //      and values of [t]. The [f] callbacks are called with a key relative
    //      to [root]. [f] is never called with an empty key for values; i.e.,
    //      folding over a value is a no-op.
    //
    //      Elements are traversed in lexical order of keys.
    //
    //      The depth is 0-indexed. If [depth] is set (by default it is not), then [f]
    //      is only called when the conditions described by the parameter is true:
    //
    //      - [Eq d] folds over nodes and contents of depth exactly [d].
    //      - [Lt d] folds over nodes and contents of depth strictly less than [d].
    //      - [Le d] folds over nodes and contents of depth less than or equal to [d].
    //      - [Gt d] folds over nodes and contents of depth strictly more than [d].
    //      - [Ge d] folds over nodes and contents of depth more than or equal to [d]. *)
    //  val fold :
    //    ?depth:[`Eq of int | `Le of int | `Lt of int | `Ge of int | `Gt of int] ->
    //    t ->
    //    key ->
    //    init:'a ->
    //    f:(key -> tree -> 'a -> 'a Lwt.t) ->
    //    'a Lwt.t

    // Tree API

    // OCaml = val hash : tree -> Context_hash.t
    fn tezedge_tree_hash(
        rt,
        tree: OCamlRef<DynBox<WorkingTreeFFI>>,
    ) -> OCaml<Result<OCamlContextHash, String>> {
        let ocaml_tree = rt.get(tree);
        let tree: &WorkingTreeFFI = ocaml_tree.borrow();

        let result = match tree.get_working_tree_root_hash()  {
            Err(err) => Err(format!("{:?}", err)),
            Ok(hash) => ContextHash::try_from(hash.as_ref()).map_err(|err| format!("{:?}", err))
        };

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
        let key: ContextKey = key.to_rust(rt);

        let result = tree.mem(&key)
            .map_err(|err| format!("{:?}", err));

        result.to_ocaml(rt)
    }

    // TODO: implement
    fn tezedge_tree_empty(
        rt,
        _unit: OCamlRef<()>,
    ) {
        OCaml::unit()
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
            NodeKind::Leaf => TreeKind::Value,
            NodeKind::NonLeaf => TreeKind::Tree,
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
        let key: ContextKey = key.to_rust(rt);

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
        let key: ContextKey = key.to_rust(rt);

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
        let key: ContextKey = key.to_rust(rt);

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
        let key: ContextKey = key.to_rust(rt);
        let value: ContextValue = value.to_rust(rt);

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
        let key: ContextKey = key.to_rust(rt);
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
        let key: ContextKey = key.to_rust(rt);

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
        let key: ContextKey = key.to_rust(rt);

        // TODO: don't clone the string, implement `ToOCaml` trait for `Rc<_>`
        let result = tree
            .list(offset, length, &key)
            .map_err(|err| format!("{:?}", err))
            .map(|v| {
                v.into_iter()
                    .map(|(s, tree)| ((*s).clone(), tree))
                    .collect::<Vec<_>>()
            });

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
        action_name: OCamlRef<String>,
        key: OCamlRef<OCamlList<String>>,
        irmin_time: f64,
        tezedge_time: f64,
    ) {
        timings::context_action(rt, action_name, key, irmin_time, tezedge_time);
        OCaml::unit()
    }
}

unsafe impl ToOCaml<DynBox<WorkingTreeFFI>> for WorkingTreeFFI {
    fn to_ocaml<'gc>(&self, rt: &'gc mut OCamlRuntime) -> OCaml<'gc, DynBox<WorkingTreeFFI>> {
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
            tezedge_context_empty,
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
            tezedge_tree_empty,
            tezedge_tree_is_empty,
            tezedge_tree_equal,
            tezedge_tree_kind,
        );
        initialize_tezedge_index_callbacks(
            tezedge_index_patch_context_get,
            tezedge_index_checkout,
            tezedge_index_exists,
            tezedge_index_close,
            tezedge_index_init,
        );
        initialize_tezedge_timing_callbacks(
            tezedge_timing_set_block,
            tezedge_timing_checkout,
            tezedge_timing_set_operation,
            tezedge_timing_commit,
            tezedge_timing_context_action,
        );
    }
}
