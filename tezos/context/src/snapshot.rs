// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{cell::RefCell, rc::Rc, sync::Arc};

use anyhow::Error;
use crypto::hash::ContextHash;
use parking_lot::RwLock;

use crate::{
    kv_store::persistent::{FileSizes, PersistentConfiguration},
    persistent::file::{File, TAG_SIZES},
    working_tree::{
        storage::Storage, string_interner::StringInterner, working_tree::WorkingTree, Commit,
        ObjectReference,
    },
    ContextKeyValueStore, IndexApi, ObjectHash, Persistent, ShellContextApi, TezedgeContext,
    TezedgeIndex,
};

pub fn reload_context_readonly(context_path: String) -> Result<Persistent, Error> {
    let sizes_file = File::<{ TAG_SIZES }>::try_new(&context_path, true)?;
    let sizes = FileSizes::make_list_from_file(&sizes_file).unwrap_or_default();
    assert!(!sizes.is_empty(), "sizes.db is invalid: {:?}", sizes);

    let mut repo = Persistent::try_new(PersistentConfiguration {
        db_path: Some(context_path),
        startup_check: true,
        read_mode: true,
    })?;
    repo.reload_database()?;

    Ok(repo)
}

/// Reads the whole tree of the commit, to extract `Storage`, that's
/// where all the objects (directories and blobs) are stored
pub fn read_commit_tree(
    ctx: Persistent,
    checkout_context_hash: &ContextHash,
) -> Result<
    (
        WorkingTree,
        Storage,
        StringInterner,
        Option<ObjectHash>,
        Commit,
    ),
    Error,
> {
    let read_repo: Arc<RwLock<ContextKeyValueStore>> = Arc::new(RwLock::new(ctx));
    let index = TezedgeIndex::new(Arc::clone(&read_repo), None);
    let context = index.checkout(checkout_context_hash)?.unwrap();

    // Take the commit from repository
    let commit: Commit = index
        .fetch_commit_from_context_hash(checkout_context_hash)?
        .unwrap();

    // If the commit has a parent, fetch it
    // It is necessary for the snapshot to have it in its db
    let parent_hash: Option<ObjectHash> = match commit.parent_commit_ref {
        Some(parent) => {
            let repo = read_repo.read();
            Some(repo.get_hash(parent)?.into_owned())
        }
        None => None,
    };

    // Traverse the tree, to store it in the `Storage`
    context.tree.traverse_working_tree(false)?;

    // Extract the `Storage`, `StringInterner` and `WorkingTree` from
    // the index
    Ok((
        Rc::try_unwrap(context.tree).ok().unwrap(),
        context.index.storage.take(),
        context.index.string_interner.take().unwrap(),
        parent_hash,
        commit,
    ))
}

/// Creates a new database to dump the context to
#[allow(clippy::too_many_arguments)]
pub fn create_new_database(
    mut tree: WorkingTree,
    mut storage: Storage,
    string_interner: StringInterner,
    parent_hash: Option<ObjectHash>,
    commit: Commit,
    snapshot_path: &str,
    checkout_context_hash: &ContextHash,
    log: fn(&str) -> (),
) -> Result<(), Error> {
    // Remove all `HashId` and `AbsoluteOffset` from the `Storage`
    // They will be recomputed
    storage.forget_references();

    // Create a new `StringInterner` that contains only the strings used
    // for this commit
    let string_interner = storage.strip_string_interner(string_interner);

    let storage = Rc::new(RefCell::new(storage));
    let string_interner = Rc::new(RefCell::new(Some(string_interner)));

    // Create the new writable repository at `snapshot_path`
    let mut write_repo = Persistent::try_new(PersistentConfiguration {
        db_path: Some(snapshot_path.into()),
        startup_check: false,
        read_mode: false,
    })?;
    write_repo.enable_hash_dedup();

    // Put the parent hash in the new repository
    let parent_ref: Option<ObjectReference> = if let Some(parent_hash) = parent_hash {
        Some(write_repo.put_hash(parent_hash)?.into())
    } else {
        None
    };

    let write_repo: Arc<RwLock<ContextKeyValueStore>> = Arc::new(RwLock::new(write_repo));

    let index = TezedgeIndex::with_storage(write_repo.clone(), storage, string_interner);

    // Make the `WorkingTree` use our new index
    tree.index = index.clone();

    {
        let now = std::time::Instant::now();
        // TODO: move these logs out
        log(" Computing context hash...");

        // Compute the hashes of the whole tree and remove the duplicate ones
        let mut repo = write_repo.write();
        tree.get_root_directory_hash(&mut *repo)?;
        index.storage.borrow_mut().deduplicate_hashes(&*repo)?;

        log(&format!(" Computing context hash ok {:?}", now.elapsed()));
    }

    let context = TezedgeContext::new(index, parent_ref, Some(Rc::new(tree)));

    let commit_context_hash = context.commit(
        commit.author.clone(),
        commit.message.clone(),
        commit.time as i64,
    )?;

    // Make sure our new context hash is the same
    assert_eq!(checkout_context_hash, &commit_context_hash);

    Ok(())
}

/// Fully read the new snapshot and re-compute all the hashes, to
/// be 100% sure that we have a valid snapshot
pub fn recompute_hashes(
    snapshot_path: &str,
    checkout_context_hash: &ContextHash,
    log: fn(&str) -> (),
) -> Result<(), Error> {
    let read_ctx = reload_context_readonly(snapshot_path.into())?;
    let read_repo: Arc<RwLock<ContextKeyValueStore>> = Arc::new(RwLock::new(read_ctx));

    let index = TezedgeIndex::new(Arc::clone(&read_repo), None);
    let mut context = index.checkout(checkout_context_hash)?.unwrap();

    let commit: Commit = index
        .fetch_commit_from_context_hash(checkout_context_hash)?
        .unwrap();
    context.parent_commit_ref = commit.parent_commit_ref;

    let now = std::time::Instant::now();
    log(" Loading in memory...");

    // Fetch all objects into `Storage`
    context.tree.traverse_working_tree(false)?;

    log(&format!(" Loading in memory ok {:?}", now.elapsed()));

    // Remove all `HashId` to re-compute them
    context.index.storage.borrow_mut().forget_references();

    let now = std::time::Instant::now();
    log(" Recomputing hashes...");

    let context_hash = context.hash(commit.author, commit.message, commit.time as i64)?;

    log(&format!(" Recomputing hashes ok {:?}", now.elapsed()));

    assert_eq!(checkout_context_hash, &context_hash);

    Ok(())
}
