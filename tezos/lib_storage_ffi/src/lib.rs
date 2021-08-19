// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use ocaml_interop::*;

pub type ContextKey = OCamlList<String>;

#[derive(PartialEq, Debug)]
pub struct ContextHash(pub Vec<u8>);

#[derive(PartialEq, Debug)]
pub struct ProtocolHash(pub Vec<u8>);

enum TaggedHash<'a> {
    Hash(&'a [u8]),
}

/// Opaque representation of the Index
pub struct TezosFfiContextIndex {}

/// Opaque representation of the Context
pub struct TezosFfiContext {}

/// Opaque representation of subtrees
pub struct TezosFfiTree {}

// Conversion between OCaml and Rust representations of hashes

unsafe impl FromOCaml<ContextHash> for ContextHash {
    fn from_ocaml(v: OCaml<ContextHash>) -> Self {
        ContextHash(unsafe { v.field::<OCamlBytes>(0).to_rust() })
    }
}

unsafe impl ToOCaml<ContextHash> for ContextHash {
    fn to_ocaml<'a>(&self, cr: &'a mut OCamlRuntime) -> OCaml<'a, ContextHash> {
        let hash = TaggedHash::Hash(&self.0);
        ocaml_alloc_variant! {
            cr, hash => {
                TaggedHash::Hash(hash: OCamlBytes)
            }
        }
    }
}

unsafe impl FromOCaml<ProtocolHash> for ProtocolHash {
    fn from_ocaml(v: OCaml<ProtocolHash>) -> Self {
        ProtocolHash(unsafe { v.field::<OCamlBytes>(0).to_rust() })
    }
}

unsafe impl ToOCaml<ProtocolHash> for ProtocolHash {
    fn to_ocaml<'a>(&self, cr: &'a mut OCamlRuntime) -> OCaml<'a, ProtocolHash> {
        let hash = TaggedHash::Hash(&self.0);
        ocaml_alloc_variant! {
            cr, hash => {
                TaggedHash::Hash(hash: OCamlBytes)
            }
        }
    }
}

// FFI function declarations
mod ffi {
    use super::{
        ContextHash, ContextKey, ProtocolHash, TezosFfiContext, TezosFfiContextIndex, TezosFfiTree,
    };
    use ocaml_interop::*;

    ocaml! {
        pub fn tezos_context_init_irmin(data_dir: String, genesis: (String, String, String), sandbox_json_patch_context: Option<(String, String)>) -> Result<(TezosFfiContextIndex, ContextHash), String>;
        pub fn tezos_context_init_tezedge(genesis: (String, String, String), sandbox_json_patch_context: Option<(String, String)>) -> Result<(TezosFfiContextIndex, ContextHash), String>;
        pub fn tezos_context_close(index: TezosFfiContextIndex);
        // Context query
        pub fn tezos_context_mem(ctxt: TezosFfiContext, key: ContextKey) -> bool;
        pub fn tezos_context_mem_tree(ctxt: TezosFfiContext, key: ContextKey) -> bool;
        pub fn tezos_context_find(ctxt: TezosFfiContext, key: ContextKey) -> Option<OCamlBytes>;
        pub fn tezos_context_find_tree(ctxt: TezosFfiContext, key: ContextKey) -> Option<TezosFfiTree>;
        pub fn tezos_context_add(ctxt: TezosFfiContext, key: ContextKey, value: OCamlBytes) -> TezosFfiContext;
        pub fn tezos_context_add_tree(ctxt: TezosFfiContext, key: ContextKey, tree: TezosFfiTree) -> TezosFfiContext;
        pub fn tezos_context_remove(ctxt: TezosFfiContext, key: ContextKey) -> TezosFfiContext;
        pub fn tezos_context_hash(time: OCamlInt64, message: Option<String>, ctxt: TezosFfiContext) -> ContextHash;
        // TODO:
        // tezos_context_list, tezos_context_fold
        // Repository
        pub fn tezos_context_checkout(idx: TezosFfiContextIndex, ctxt_hash: ContextHash) -> Option<TezosFfiContext>;
        pub fn tezos_context_commit(time: OCamlInt64, message: String, ctxt: TezosFfiContext) -> ContextHash;
        // Tree
        pub fn tezos_context_tree_mem(tree: TezosFfiTree, key: ContextKey) -> bool;
        pub fn tezos_context_tree_mem_tree(tree: TezosFfiTree, key: ContextKey) -> bool;
        pub fn tezos_context_tree_find(tree: TezosFfiTree, key: ContextKey) -> Option<OCamlBytes>;
        pub fn tezos_context_tree_find_tree(tree: TezosFfiTree, key: ContextKey) -> Option<TezosFfiTree>;
        pub fn tezos_context_tree_add(tree: TezosFfiTree, key: ContextKey, value: OCamlBytes) -> TezosFfiTree;
        pub fn tezos_context_tree_add_tree(tree: TezosFfiTree, key: ContextKey, subtree: TezosFfiTree) -> TezosFfiTree;
        pub fn tezos_context_tree_remove(tree: TezosFfiTree, key: ContextKey) -> TezosFfiTree;
        pub fn tezos_context_tree_empty(ctxt: TezosFfiContext) -> TezosFfiTree;
        pub fn tezos_context_tree_is_empty(tree: TezosFfiTree) -> bool;
        pub fn tezos_context_tree_hash(tree: TezosFfiTree) -> ContextHash;
        // TODO:
        // tezos_context_tree_list, tezos_context_tree_fold
        // Special
        pub fn tezos_context_get_protocol(ctxt: TezosFfiContext) -> ProtocolHash;
        pub fn tezos_context_add_protocol(ctxt: TezosFfiContext, proto_hash: ProtocolHash) -> TezosFfiContext;
        // TODO:
        // tezos_context_get_test_chain, tezos_context_add_test_chain, tezos_context_remove_test_chain
    }
}

// Context operations

pub mod context {
    use super::*;

    pub fn init_irmin(
        cr: &mut OCamlRuntime,
        data_dir: &str,
        genesis: (String, String, String),
        sandbox_json_patch_context: Option<(String, String)>,
    ) -> Result<(BoxRoot<TezosFfiContextIndex>, ContextHash), String> {
        let data_dir = data_dir.to_boxroot(cr);
        let genesis = genesis.to_boxroot(cr);
        let sandbox_json_patch_context = sandbox_json_patch_context.to_boxroot(cr);
        let result =
            ffi::tezos_context_init_irmin(cr, &data_dir, &genesis, &sandbox_json_patch_context);
        match cr.get(&result).to_result() {
            Ok(result) => Ok((BoxRoot::new(result.fst()), result.snd().to_rust())),
            Err(err) => Err(err.to_rust()),
        }
    }

    pub fn init_tezedge(
        cr: &mut OCamlRuntime,
        genesis: (String, String, String),
        sandbox_json_patch_context: Option<(String, String)>,
    ) -> Result<(BoxRoot<TezosFfiContextIndex>, ContextHash), String> {
        let genesis = genesis.to_boxroot(cr);
        let sandbox_json_patch_context = sandbox_json_patch_context.to_boxroot(cr);
        let result = ffi::tezos_context_init_tezedge(cr, &genesis, &sandbox_json_patch_context);
        match cr.get(&result).to_result() {
            Ok(result) => Ok((BoxRoot::new(result.fst()), result.snd().to_rust())),
            Err(err) => Err(err.to_rust()),
        }
    }

    pub fn close(cr: &mut OCamlRuntime, index: OCamlRef<TezosFfiContextIndex>) {
        ffi::tezos_context_close(cr, index);
    }

    // Context query

    pub fn mem(cr: &mut OCamlRuntime, ctxt: OCamlRef<TezosFfiContext>, key: &Vec<String>) -> bool {
        let key = key.to_boxroot(cr);
        ffi::tezos_context_mem(cr, ctxt, &key).to_rust(cr)
    }

    pub fn mem_tree(
        cr: &mut OCamlRuntime,
        ctxt: OCamlRef<TezosFfiContext>,
        key: &Vec<String>,
    ) -> bool {
        let key = key.to_boxroot(cr);
        ffi::tezos_context_mem_tree(cr, ctxt, &key).to_rust(cr)
    }

    pub fn find(
        cr: &mut OCamlRuntime,
        ctxt: OCamlRef<TezosFfiContext>,
        key: &Vec<String>,
    ) -> Option<Vec<u8>> {
        let key = key.to_boxroot(cr);
        ffi::tezos_context_find(cr, ctxt, &key).to_rust(cr)
    }

    pub fn find_tree(
        cr: &mut OCamlRuntime,
        ctxt: OCamlRef<TezosFfiContext>,
        key: &Vec<String>,
    ) -> Option<BoxRoot<TezosFfiTree>> {
        let key = key.to_boxroot(cr);
        let result = ffi::tezos_context_find_tree(cr, ctxt, &key);
        cr.get(&result).to_option().map(BoxRoot::new)
    }

    pub fn add(
        cr: &mut OCamlRuntime,
        ctxt: OCamlRef<TezosFfiContext>,
        key: &Vec<String>,
        value: &[u8],
    ) -> BoxRoot<TezosFfiContext> {
        let key = key.to_boxroot(cr);
        let value = value.to_boxroot(cr);
        ffi::tezos_context_add(cr, ctxt, &key, &value)
    }

    pub fn add_tree(
        cr: &mut OCamlRuntime,
        ctxt: OCamlRef<TezosFfiContext>,
        key: &Vec<String>,
        tree: OCamlRef<TezosFfiTree>,
    ) -> BoxRoot<TezosFfiContext> {
        let key = key.to_boxroot(cr);
        ffi::tezos_context_add_tree(cr, ctxt, &key, tree)
    }

    pub fn remove(
        cr: &mut OCamlRuntime,
        ctxt: OCamlRef<TezosFfiContext>,
        key: &Vec<String>,
    ) -> BoxRoot<TezosFfiContext> {
        let key = key.to_boxroot(cr);
        ffi::tezos_context_remove(cr, ctxt, &key)
    }

    pub fn hash(
        cr: &mut OCamlRuntime,
        time: i64,
        message: Option<String>,
        ctxt: OCamlRef<TezosFfiContext>,
    ) -> ContextHash {
        let time = time.to_boxroot(cr);
        let message = message.to_boxroot(cr);
        ffi::tezos_context_hash(cr, &time, &message, ctxt).to_rust(cr)
    }

    // Repository

    pub fn checkout(
        cr: &mut OCamlRuntime,
        ctxt_idx: OCamlRef<TezosFfiContextIndex>,
        hash: &ContextHash,
    ) -> Option<BoxRoot<TezosFfiContext>> {
        let hash = hash.to_boxroot(cr);
        let result = ffi::tezos_context_checkout(cr, ctxt_idx, &hash);
        cr.get(&result).to_option().map(BoxRoot::new)
    }

    pub fn commit(
        cr: &mut OCamlRuntime,
        time: i64,
        message: &str,
        ctxt: OCamlRef<TezosFfiContext>,
    ) -> ContextHash {
        let time = time.to_boxroot(cr);
        let message = message.to_boxroot(cr);
        ffi::tezos_context_commit(cr, &time, &message, ctxt).to_rust(cr)
    }

    // Special

    pub fn get_protocol(cr: &mut OCamlRuntime, ctxt: OCamlRef<TezosFfiContext>) -> Vec<u8> {
        let proto_hash: ProtocolHash = ffi::tezos_context_get_protocol(cr, ctxt).to_rust(cr);
        proto_hash.0
    }

    pub fn add_protocol(
        cr: &mut OCamlRuntime,
        ctxt: OCamlRef<TezosFfiContext>,
        value: &[u8],
    ) -> BoxRoot<TezosFfiContext> {
        let value = ProtocolHash(value.into());
        let value = value.to_boxroot(cr);
        ffi::tezos_context_add_protocol(cr, ctxt, &value)
    }
}

// Subtree operations

pub mod tree {
    use super::*;

    pub fn empty(cr: &mut OCamlRuntime, ctxt: OCamlRef<TezosFfiContext>) -> BoxRoot<TezosFfiTree> {
        ffi::tezos_context_tree_empty(cr, &ctxt)
    }

    pub fn is_empty(cr: &mut OCamlRuntime, tree: OCamlRef<TezosFfiTree>) -> bool {
        ffi::tezos_context_tree_is_empty(cr, &tree).to_rust(cr)
    }

    pub fn mem(cr: &mut OCamlRuntime, tree: OCamlRef<TezosFfiTree>, key: &Vec<String>) -> bool {
        let key = key.to_boxroot(cr);
        ffi::tezos_context_tree_mem(cr, tree, &key).to_rust(cr)
    }

    pub fn mem_tree(
        cr: &mut OCamlRuntime,
        tree: OCamlRef<TezosFfiTree>,
        key: &Vec<String>,
    ) -> bool {
        let key = key.to_boxroot(cr);
        ffi::tezos_context_tree_mem_tree(cr, tree, &key).to_rust(cr)
    }

    pub fn find(
        cr: &mut OCamlRuntime,
        tree: OCamlRef<TezosFfiTree>,
        key: &Vec<String>,
    ) -> Option<Vec<u8>> {
        let key = key.to_boxroot(cr);
        ffi::tezos_context_tree_find(cr, tree, &key).to_rust(cr)
    }

    pub fn find_tree(
        cr: &mut OCamlRuntime,
        tree: OCamlRef<TezosFfiTree>,
        key: &Vec<String>,
    ) -> Option<BoxRoot<TezosFfiTree>> {
        let key = key.to_boxroot(cr);
        let result = ffi::tezos_context_tree_find_tree(cr, tree, &key);
        cr.get(&result).to_option().map(BoxRoot::new)
    }

    pub fn add(
        cr: &mut OCamlRuntime,
        tree: OCamlRef<TezosFfiTree>,
        key: &Vec<String>,
        value: &[u8],
    ) -> BoxRoot<TezosFfiTree> {
        let key = key.to_boxroot(cr);
        let value = value.to_boxroot(cr);
        ffi::tezos_context_tree_add(cr, tree, &key, &value)
    }

    pub fn add_tree(
        cr: &mut OCamlRuntime,
        tree: OCamlRef<TezosFfiTree>,
        key: &Vec<String>,
        subtree: OCamlRef<TezosFfiTree>,
    ) -> BoxRoot<TezosFfiTree> {
        let key = key.to_boxroot(cr);
        ffi::tezos_context_tree_add_tree(cr, tree, &key, subtree)
    }

    pub fn remove(
        cr: &mut OCamlRuntime,
        tree: OCamlRef<TezosFfiTree>,
        key: &Vec<String>,
    ) -> BoxRoot<TezosFfiTree> {
        let key = key.to_boxroot(cr);
        ffi::tezos_context_tree_remove(cr, tree, &key)
    }

    pub fn hash(cr: &mut OCamlRuntime, tree: OCamlRef<TezosFfiTree>) -> ContextHash {
        ffi::tezos_context_tree_hash(cr, tree).to_rust(cr)
    }
}

#[cfg(test)]
macro_rules! key {
    ($key:expr) => {{
        $key.split('/').map(String::from).collect::<Vec<String>>()
    }};
    ($($arg:tt)*) => {{
        key!(format!($($arg)*))
    }};
}

#[cfg(test)]
fn test_context_inodes(
    cr: &mut OCamlRuntime,
    tezedge_index: OCamlRef<TezosFfiContextIndex>,
    tezedge_genesis_hash: &ContextHash,
    irmin_index: OCamlRef<TezosFfiContextIndex>,
    irmin_genesis_hash: &ContextHash,
) {
    use std::time::SystemTime;

    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut tezedge_ctxt = context::checkout(cr, &tezedge_index, tezedge_genesis_hash).unwrap();
    let mut irmin_ctxt = context::checkout(cr, &irmin_index, &irmin_genesis_hash).unwrap();

    let tezedge_commit_hash_init = context::commit(cr, time as i64, &"commit", &tezedge_ctxt);
    let irmin_commit_hash_init = context::commit(cr, time as i64, &"commit", &irmin_ctxt);

    for index in 0..2_000 {
        let key = key!(format!("root/{}", index));

        tezedge_ctxt = context::add(cr, &tezedge_ctxt, &key, "".as_bytes());
        irmin_ctxt = context::add(cr, &irmin_ctxt, &key, "".as_bytes());

        let tezedge_commit_hash = context::commit(cr, time as i64, &"commit", &tezedge_ctxt);
        let irmin_commit_hash = context::commit(cr, time as i64, &"commit", &irmin_ctxt);

        assert_eq!(irmin_commit_hash, tezedge_commit_hash);
    }

    for index in 0..2_000 {
        let key = key!(format!("root/{}", index));

        tezedge_ctxt = context::remove(cr, &tezedge_ctxt, &key);
        irmin_ctxt = context::remove(cr, &irmin_ctxt, &key);

        let tezedge_commit_hash = context::commit(cr, time as i64, &"commit", &tezedge_ctxt);
        let irmin_commit_hash = context::commit(cr, time as i64, &"commit", &irmin_ctxt);

        assert_eq!(irmin_commit_hash, tezedge_commit_hash);
    }

    let tezedge_commit_hash_end = context::commit(cr, time as i64, &"commit", &tezedge_ctxt);
    let irmin_commit_hash_end = context::commit(cr, time as i64, &"commit", &irmin_ctxt);
    assert_eq!(irmin_commit_hash_end, tezedge_commit_hash_end);

    assert_eq!(irmin_commit_hash_init, irmin_commit_hash_end);
    assert_eq!(tezedge_commit_hash_init, tezedge_commit_hash_end);
}

#[test]
fn test_context_calls() {
    use std::time::SystemTime;
    use tempfile::tempdir;

    // Initialize the persistent OCaml runtime and initialize callbacks
    ocaml_interop::OCamlRuntime::init_persistent();
    tezos_context::ffi::initialize_callbacks();

    // OCaml runtime handle for FFI calls
    let cr = unsafe { ocaml_interop::OCamlRuntime::recover_handle() };

    let irmin_dir = tempdir().unwrap();
    let genesis = (
        "2019-08-06T15:18:56Z".to_string(),
        "BLockGenesisGenesisGenesisGenesisGenesiscde8db4cX94".to_string(),
        "PtBMwNZT94N7gXKw4i273CKcSaBrrBnqnt3RATExNKr9KNX2USV".to_string(),
    );

    let (tezedge_index, tezedge_genesis_hash) =
        context::init_tezedge(cr, genesis.clone(), None).unwrap();
    let (irmin_index, irmin_genesis_hash) = context::init_irmin(
        cr,
        irmin_dir.path().to_str().unwrap(),
        genesis.clone(),
        None,
    )
    .unwrap();

    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    test_context_inodes(
        cr,
        &tezedge_index,
        &tezedge_genesis_hash,
        &irmin_index,
        &irmin_genesis_hash,
    );

    let tezedge_ctxt = context::checkout(cr, &tezedge_index, &tezedge_genesis_hash).unwrap();
    let tezedge_ctxt = context::add(cr, &tezedge_ctxt, &key!("empty/value"), "".as_bytes());
    let tezedge_ctxt = context::add(cr, &tezedge_ctxt, &key!("some/path"), "value".as_bytes());
    let tezedge_ctxt = context::remove(cr, &tezedge_ctxt, &key!("some/path/nested"));
    let tezedge_ctxt = context::add(cr, &tezedge_ctxt, &key!("some/path2"), "value".as_bytes());
    let tezedge_ctxt = context::remove(cr, &tezedge_ctxt, &key!("some/path2"));
    let tezedge_ctxt = context::add(cr, &tezedge_ctxt, &key!("some/path3"), "value".as_bytes());
    let tezedge_empty_tree = tree::empty(cr, &tezedge_ctxt);
    let tezedge_ctxt =
        context::add_tree(cr, &tezedge_ctxt, &key!("some/path3"), &tezedge_empty_tree);
    let tezedge_ctxt = context::add(
        cr,
        &tezedge_ctxt,
        &key!("some/path4/nest"),
        "value".as_bytes(),
    );
    let tezedge_ctxt = context::add(cr, &tezedge_ctxt, &key!("some/path4"), "value".as_bytes());
    let tezedge_ctxt_hash = context::hash(cr, time as i64, None, &tezedge_ctxt);
    let tezedge_commit_hash = context::commit(cr, time as i64, &"commit", &tezedge_ctxt);

    let irmin_ctxt = context::checkout(cr, &irmin_index, &irmin_genesis_hash).unwrap();
    let irmin_ctxt = context::add(cr, &irmin_ctxt, &key!("empty/value"), "".as_bytes());
    let irmin_ctxt = context::add(cr, &irmin_ctxt, &key!("some/path"), "value".as_bytes());
    let irmin_ctxt = context::remove(cr, &irmin_ctxt, &key!("some/path/nested"));
    let irmin_ctxt = context::add(cr, &irmin_ctxt, &key!("some/path2"), "value".as_bytes());
    let irmin_ctxt = context::remove(cr, &irmin_ctxt, &key!("some/path2"));
    let irmin_ctxt = context::add(cr, &irmin_ctxt, &key!("some/path3"), "value".as_bytes());
    let irmin_empty_tree = tree::empty(cr, &irmin_ctxt);
    let irmin_ctxt = context::add_tree(cr, &irmin_ctxt, &key!("some/path3"), &irmin_empty_tree);
    let irmin_ctxt = context::add(
        cr,
        &irmin_ctxt,
        &key!("some/path4/nest"),
        "value".as_bytes(),
    );
    let irmin_ctxt = context::add(cr, &irmin_ctxt, &key!("some/path4"), "value".as_bytes());
    let irmin_ctxt_hash = context::hash(cr, time as i64, None, &irmin_ctxt);
    let irmin_commit_hash = context::commit(cr, time as i64, &"commit", &irmin_ctxt);

    assert_eq!(irmin_commit_hash, tezedge_commit_hash);
    assert_eq!(tezedge_ctxt_hash, irmin_ctxt_hash);

    let tezedge_ctxt = context::checkout(cr, &tezedge_index, &tezedge_commit_hash);

    assert!(tezedge_ctxt.is_some());

    let irmin_ctxt = context::checkout(cr, &irmin_index, &irmin_commit_hash);

    assert!(irmin_ctxt.is_some());

    let tezedge_ctxt = tezedge_ctxt.unwrap();
    let irmin_ctxt = irmin_ctxt.unwrap();

    assert!(context::mem(cr, &tezedge_ctxt, &key!("some/path")));
    assert!(context::mem_tree(cr, &tezedge_ctxt, &key!("some/path")));
    assert!(!context::mem(cr, &tezedge_ctxt, &key!("some/path2")));
    assert!(!context::mem(cr, &tezedge_ctxt, &key!("some")));
    assert!(context::mem_tree(cr, &tezedge_ctxt, &key!("some")));
    assert!(context::mem(cr, &irmin_ctxt, &key!("some/path")));
    assert!(context::mem_tree(cr, &irmin_ctxt, &key!("some/path")));
    assert!(!context::mem(cr, &irmin_ctxt, &key!("some/path2")));
    assert!(!context::mem(cr, &irmin_ctxt, &key!("some")));
    assert!(context::mem_tree(cr, &irmin_ctxt, &key!("some")));

    let tezedge_tree = tree::empty(cr, &tezedge_ctxt);

    assert!(tree::is_empty(cr, &tezedge_tree));

    let tezedge_tree = tree::add(cr, &tezedge_tree, &key!("some/path"), "value".as_bytes());
    let tezedge_tree = tree::add(cr, &tezedge_tree, &key!("some/path2"), "value".as_bytes());
    let tezedge_tree = tree::remove(cr, &tezedge_tree, &key!("some/path2"));

    assert!(tree::mem(cr, &tezedge_tree, &key!("some/path")));
    assert!(!tree::mem(cr, &tezedge_tree, &key!("some/path2")));
    assert!(!tree::is_empty(cr, &tezedge_tree));

    let irmin_tree = tree::empty(cr, &irmin_ctxt);

    assert!(tree::is_empty(cr, &irmin_tree));

    let irmin_tree = tree::add(cr, &irmin_tree, &key!("some/path"), "value".as_bytes());
    let irmin_tree = tree::add(cr, &irmin_tree, &key!("some/path2"), "value".as_bytes());
    let irmin_tree = tree::remove(cr, &irmin_tree, &key!("some/path2"));

    assert!(tree::mem(cr, &irmin_tree, &key!("some/path")));
    assert!(!tree::mem(cr, &irmin_tree, &key!("some/path2")));
    assert!(!tree::is_empty(cr, &irmin_tree));

    let tezedge_ctxt = context::add_tree(cr, &tezedge_ctxt, &key!("tree"), &tezedge_tree);
    let irmin_ctxt = context::add_tree(cr, &irmin_ctxt, &key!("tree"), &irmin_tree);

    let tezedge_commit_hash = context::commit(cr, time as i64, &"commit", &tezedge_ctxt);
    let irmin_commit_hash = context::commit(cr, time as i64, &"commit", &irmin_ctxt);

    assert_eq!(tezedge_commit_hash, irmin_commit_hash);

    let tezedge_ctxt = context::checkout(cr, &tezedge_index, &tezedge_commit_hash);

    assert!(tezedge_ctxt.is_some());

    let irmin_ctxt = context::checkout(cr, &irmin_index, &irmin_commit_hash);

    assert!(irmin_ctxt.is_some());

    let tezedge_ctxt = tezedge_ctxt.unwrap();

    assert!(context::find_tree(cr, &tezedge_ctxt, &key!("tree/some")).is_some());
    assert!(context::find_tree(cr, &tezedge_ctxt, &key!("tree/some/nonexistent")).is_none());
    assert!(context::find_tree(cr, &tezedge_ctxt, &key!("tree/some/path")).is_some());
    assert!(context::find_tree(cr, &tezedge_ctxt, &key!("tree")).is_some());
    assert!(context::find_tree(cr, &tezedge_ctxt, &key!("nonexistent")).is_none());
    assert!(context::find_tree(cr, &tezedge_ctxt, &vec![]).is_some());
    assert!(context::find(cr, &tezedge_ctxt, &key!("tree")).is_none());
    assert!(!context::mem(cr, &tezedge_ctxt, &key!("tree/some/path2")));
    let tv = context::find_tree(cr, &tezedge_ctxt, &key!("tree/some/path")).unwrap();
    assert!(!tree::is_empty(cr, &tv));
    assert!(!context::mem(cr, &tezedge_ctxt, &key!("tree/some")));
    assert!(context::mem(cr, &tezedge_ctxt, &key!("tree/some/path")));

    let irmin_ctxt = irmin_ctxt.unwrap();

    assert!(context::find_tree(cr, &irmin_ctxt, &key!("tree/some")).is_some());
    assert!(context::find_tree(cr, &irmin_ctxt, &key!("tree/some/nonexistent")).is_none());
    assert!(context::find_tree(cr, &irmin_ctxt, &key!("tree/some/path")).is_some());
    assert!(context::find_tree(cr, &irmin_ctxt, &key!("tree")).is_some());
    assert!(context::find_tree(cr, &irmin_ctxt, &key!("nonexistent")).is_none());
    assert!(context::find_tree(cr, &irmin_ctxt, &vec![]).is_some());
    assert!(context::find(cr, &irmin_ctxt, &key!("tree")).is_none());
    assert!(!context::mem(cr, &irmin_ctxt, &key!("tree/some/path2")));
    let tv = context::find_tree(cr, &irmin_ctxt, &key!("tree/some/path")).unwrap();
    assert!(!tree::is_empty(cr, &tv));
    assert!(!context::mem(cr, &irmin_ctxt, &key!("tree/some")));
    assert!(context::mem(cr, &irmin_ctxt, &key!("tree/some/path")));

    let tezedge_tree = context::find_tree(cr, &tezedge_ctxt, &key!("tree")).unwrap();
    let irmin_tree = context::find_tree(cr, &irmin_ctxt, &key!("tree")).unwrap();
    let tezedge_ctxt = context::add_tree(cr, &tezedge_ctxt, &key!("copy/path/tree"), &tezedge_tree);
    let irmin_ctxt = context::add_tree(cr, &irmin_ctxt, &key!("copy/path/tree"), &irmin_tree);

    assert_eq!(
        context::hash(cr, 1, None, &tezedge_ctxt),
        context::hash(cr, 1, None, &irmin_ctxt)
    );
}
