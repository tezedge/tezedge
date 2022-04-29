// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{path::PathBuf, sync::Arc};

use crate::log::print;
use clap::{Parser, Subcommand};
use crypto::hash::{ContextHash, HashTrait};
use parking_lot::RwLock;
use tezos_context::{
    kv_store::persistent::{FileSizes, PersistentConfiguration},
    persistent::file::{File, TAG_SIZES},
    snapshot,
    working_tree::string_interner::StringId,
    IndexApi, Persistent, TezedgeIndex,
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

    let repo = snapshot::reload_context_readonly(context_path).unwrap();

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
            let context_hash = {
                let ctx = reload_context_readonly(context_path.clone());
                ctx.get_last_context_hash().unwrap()
            };

            snapshot::recompute_hashes(&context_path, &context_hash, log_snapshot).unwrap();

            log!(
                "Context at {:?} for {:?} is valid",
                context_path,
                context_hash
            );
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

            let repo = context.index.repository.read();
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

            let now = std::time::Instant::now();
            log!("Loading context in memory...");

            let (tree, storage, string_interner, parent_hash, commit) =
                snapshot::read_commit_tree(ctx, &checkout_context_hash).unwrap();

            log!("Loading context in memory ok {:?}", now.elapsed());

            let now = std::time::Instant::now();
            log!("Creating snapshot from context in memory...");

            snapshot::create_new_database(
                tree,
                storage,
                string_interner,
                parent_hash,
                commit,
                &snapshot_path,
                &checkout_context_hash,
                log_snapshot,
            )
            .unwrap();

            log!(
                "Creating snapshot from context in memory ok {:?}",
                now.elapsed()
            );

            let now = std::time::Instant::now();
            log!("Loading snapshot & re-compute hashes...");

            snapshot::recompute_hashes(&snapshot_path, &checkout_context_hash, log_snapshot)
                .unwrap();

            log!(
                "Loading snapshot & re-compute hashes ok {:?}",
                now.elapsed()
            );

            log!(
                "Snapshot {:?} created in {:?}",
                snapshot_path,
                start.elapsed(),
            );
        }
    }
}

fn log_snapshot(s: &str) {
    log!("{}", s)
}
