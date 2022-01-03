// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, RwLock},
};

use crate::log::print;
use clap::{Parser, Subcommand};
use crypto::hash::{ContextHash, HashTrait};
use tezos_context::{
    kv_store::persistent::{FileSizes, PersistentConfiguration},
    persistent::file::{File, TAG_SIZES},
    working_tree::{string_interner::StringId, Commit, ObjectReference},
    ContextKeyValueStore, IndexApi, ObjectHash, Persistent, ShellContextApi, TezedgeContext,
    TezedgeIndex,
};

#[macro_use]
mod log;
mod stats;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build integrity database (file `sizes.db`)
    BuildIntegrity {
        /// Path of the persistent context
        #[clap(short, long)]
        context_path: String,
        #[clap(short, long)]
        /// Path to write the resulting `sizes.db`, default to current directory
        output_dir: Option<String>,
    },
    /// Check if the context match its `sizes.db`
    IsValidContext {
        /// Path of the persistent context
        #[clap(short, long)]
        context_path: String,
    },
    /// Display `sizes.db` file
    DumpChecksums {
        /// Path of the persistent context
        #[clap(short, long)]
        context_path: String,
    },
    /// Display sizes of the different objects for the selected commit
    ContextSize {
        /// Path of the persistent context
        #[clap(short, long)]
        context_path: String,
        /// Context hash to inspect, default to last commit
        #[clap(short, long)]
        hash: Option<String>,
    },
    /// Create a snapshot from a commit.
    /// This will create a new context with all unused objects removed
    MakeSnapshot {
        /// Path of the persistent context
        #[clap(short, long)]
        context_path: String,
        /// Context hash to make the snapshot from, default to last commit
        #[clap(short, long)]
        hash: Option<String>,
        /// Path of the result, default to `/tmp/tezedge_snapshot_XXX`
        #[clap(short, long)]
        output: Option<String>,
    },
}

fn reload_context_readonly(context_path: String) -> Persistent {
    log!(" Validating context {:?}...", context_path);

    let now = std::time::Instant::now();

    let sizes_file = File::<{ TAG_SIZES }>::try_new(&context_path, true).unwrap();
    let sizes = FileSizes::make_list_from_file(&sizes_file).unwrap_or_default();
    assert!(!sizes.is_empty(), "sizes.db is invalid: {:?}", sizes);

    let mut repo = Persistent::try_new(PersistentConfiguration {
        db_path: Some(context_path),
        startup_check: true,
        read_mode: true,
    })
    .unwrap();
    repo.reload_database().unwrap();

    log!(" Validating context ok {:?}", now.elapsed());

    repo
}

fn create_snapshot_path(base_path: Option<String>) -> String {
    match base_path {
        Some(path_str) => {
            let path = PathBuf::from(&path_str);
            if path.exists() {
                elog!("{:?} already exist", path);
            }
            path_str
        }
        None => {
            for index in 0.. {
                let path_str = format!("/tmp/tezedge_snapshot_{}", index);
                let path = PathBuf::from(&path_str);
                if path.exists() {
                    continue;
                }
                return path_str;
            }
            unreachable!()
        }
    }
}

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::DumpChecksums { context_path } => {
            let sizes_file = File::<{ TAG_SIZES }>::try_new(&context_path, true).unwrap();
            let sizes = FileSizes::make_list_from_file(&sizes_file).unwrap_or_default();
            log!("checksums={:#?}", sizes);
        }
        Commands::BuildIntegrity {
            context_path,
            output_dir,
        } => {
            let output_dir = output_dir.unwrap_or_else(|| "".to_string());

            // Make sure `sizes.db` doesn't already exist
            match File::<{ TAG_SIZES }>::create_new_file(&output_dir) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    panic!(
                        "The resulting file `sizes.db` already exist at `{:?}`",
                        output_dir
                    );
                }
                Err(e) => panic!("{:?}", e),
            };

            let mut ctx = Persistent::try_new(PersistentConfiguration {
                db_path: Some(context_path),
                startup_check: true,
                read_mode: true,
            })
            .unwrap();
            let mut output_file = File::<{ TAG_SIZES }>::try_new(&output_dir, false).unwrap();

            ctx.compute_integrity(&mut output_file).unwrap();

            let sizes = ctx.get_file_sizes();
            log!("Result={:#?}", sizes);
        }
        Commands::IsValidContext { context_path } => {
            reload_context_readonly(context_path);
        }
        Commands::ContextSize {
            context_path,
            hash: context_hash,
        } => {
            let ctx = reload_context_readonly(context_path);

            let context_hash = if let Some(context_hash) = context_hash.as_ref() {
                ContextHash::from_b58check(context_hash).unwrap()
            } else {
                ctx.get_last_context_hash().unwrap()
            };

            log!("Computing size for {:?}", context_hash.to_base58_check());

            let index = TezedgeIndex::new(Arc::new(RwLock::new(ctx)), None);

            let now = std::time::Instant::now();

            let context = index.checkout(&context_hash).unwrap().unwrap();
            let stats = context.tree.traverse_working_tree(true).unwrap();

            let repo = context.index.repository.read().unwrap();
            let repo_stats = repo.get_read_statistics().unwrap().unwrap();

            let mut stats = stats.unwrap();
            stats.objects_total_bytes = repo_stats.objects_total_bytes;
            stats.lowest_offset = repo_stats.lowest_offset;
            stats.nshapes = repo_stats.unique_shapes.len();
            stats.shapes_total_bytes = repo_stats.shapes_length * std::mem::size_of::<StringId>();

            log!(
                "Context size: {:#?}",
                stats::DebugWorkingTreeStatistics(stats)
            );
            log!("Total Time {:?}", now.elapsed());
        }
        Commands::MakeSnapshot {
            context_path,
            hash: context_hash,
            output,
        } => {
            let start = std::time::Instant::now();

            let snapshot_path = create_snapshot_path(output);

            log!("Start creating snapshot at {:?}", snapshot_path,);

            let ctx = reload_context_readonly(context_path);

            let checkout_context_hash: ContextHash =
                if let Some(context_hash) = context_hash.as_ref() {
                    ContextHash::from_b58check(context_hash).unwrap()
                } else {
                    ctx.get_last_context_hash().unwrap()
                };

            log!("Using {:?}", checkout_context_hash);

            let (mut tree, mut storage, string_interner, parent_hash, commit) = {
                // This block reads the whole tree of the commit, to extract `Storage`, that's
                // where all the objects (directories and blobs) are stored

                let now = std::time::Instant::now();
                log!("Loading context in memory...");

                let read_repo: Arc<RwLock<ContextKeyValueStore>> = Arc::new(RwLock::new(ctx));
                let index = TezedgeIndex::new(Arc::clone(&read_repo), None);
                let context = index.checkout(&checkout_context_hash).unwrap().unwrap();

                // Take the commit from repository
                let commit: Commit = index
                    .fetch_commit_from_context_hash(&checkout_context_hash)
                    .unwrap()
                    .unwrap();

                // If the commit has a parent, fetch it
                // It is necessary for the snapshot to have it in its db
                let parent_hash: Option<ObjectHash> = match commit.parent_commit_ref {
                    Some(parent) => {
                        let repo = read_repo.read().unwrap();
                        Some(repo.get_hash(parent).unwrap().into_owned())
                    }
                    None => None,
                };

                // Traverse the tree, to store it in the `Storage`
                context.tree.traverse_working_tree(false).unwrap();

                log!("Loading context in memory ok {:?}", now.elapsed());

                // Extract the `Storage`, `StringInterner` and `WorkingTree` from
                // the index
                (
                    Rc::try_unwrap(context.tree).ok().unwrap(),
                    context.index.storage.take(),
                    context.index.string_interner.take().unwrap(),
                    parent_hash,
                    commit,
                )
            };

            {
                // This block creates the new database

                let now = std::time::Instant::now();
                log!("Creating snapshot from context in memory...");

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
                    db_path: Some(snapshot_path.clone()),
                    startup_check: false,
                    read_mode: false,
                })
                .unwrap();
                write_repo.enable_hash_dedup();

                // Put the parent hash in the new repository
                let parent_ref: Option<ObjectReference> =
                    parent_hash.map(|parent_hash| write_repo.put_hash(parent_hash).unwrap().into());

                let write_repo: Arc<RwLock<ContextKeyValueStore>> =
                    Arc::new(RwLock::new(write_repo));

                let index =
                    TezedgeIndex::with_storage(write_repo.clone(), storage, string_interner);

                // Make the `WorkingTree` use our new index
                tree.index = index.clone();

                {
                    let now = std::time::Instant::now();
                    log!(" Computing context hash...");

                    // Compute the hashes of the whole tree and remove the duplicate ones
                    let mut repo = write_repo.write().unwrap();
                    tree.get_root_directory_hash(&mut *repo).unwrap();
                    index.storage.borrow_mut().deduplicate_hashes(&*repo);

                    log!(" Computing context hash ok {:?}", now.elapsed());
                }

                let context = TezedgeContext::new(index, parent_ref, Some(Rc::new(tree)));

                let commit_context_hash = context
                    .commit(
                        commit.author.clone(),
                        commit.message.clone(),
                        commit.time as i64,
                    )
                    .unwrap();

                // Make sure our new context hash is the same
                assert_eq!(checkout_context_hash, commit_context_hash);

                log!(
                    "Creating snapshot from context in memory ok {:?}",
                    now.elapsed()
                );
            }

            {
                // Fully read the new snapshot and re-compute all the hashes, to
                // be 100% sure that we have a valid snapshot

                let start = std::time::Instant::now();
                log!("Loading snapshot & re-compute hashes...");

                let read_ctx = reload_context_readonly(snapshot_path.clone());
                let read_repo: Arc<RwLock<ContextKeyValueStore>> = Arc::new(RwLock::new(read_ctx));

                let index = TezedgeIndex::new(Arc::clone(&read_repo), None);
                let mut context = index.checkout(&checkout_context_hash).unwrap().unwrap();

                let commit: Commit = index
                    .fetch_commit_from_context_hash(&checkout_context_hash)
                    .unwrap()
                    .unwrap();
                context.parent_commit_ref = commit.parent_commit_ref;

                let now = std::time::Instant::now();
                log!(" Loading in memory...");

                // Fetch all objects into `Storage`
                context.tree.traverse_working_tree(false).unwrap();

                log!(" Loading in memory ok {:?}", now.elapsed());

                // Remove all `HashId` to re-compute them
                context.index.storage.borrow_mut().forget_references();

                let now = std::time::Instant::now();
                log!(" Recomputing hashes...");

                let context_hash = context
                    .hash(commit.author, commit.message, commit.time as i64)
                    .unwrap();

                log!(" Recomputing hashes ok {:?}", now.elapsed());

                assert_eq!(checkout_context_hash, context_hash);

                log!(
                    "Loading snapshot & re-compute hashes ok {:?}",
                    start.elapsed()
                );
            }

            log!(
                "Snapshot {:?} created in {:?}",
                snapshot_path,
                start.elapsed(),
            );
        }
    }
}
