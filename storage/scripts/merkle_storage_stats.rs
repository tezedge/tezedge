// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use clap::{App, Arg};
use std::convert::TryInto;
use std::fs::File;
use std::io::Read;

use backend::{BTreeMapBackend, InMemoryBackend, KVStoreGCed, MarkSweepGCed};
use merkle_storage::{check_commit_hashes, EntryHash, MerkleStorage};
use storage::*;
use storage_backend::StorageBackend;

use crypto::hash::MerkleHash;
use std::convert::TryFrom;
use storage::action_file::ActionsFileReader;
use tezos_context::channel::ContextAction;

pub fn get_tree_hash(action: &ContextAction) -> Option<MerkleHash> {
    match &action {
        ContextAction::Get { tree_hash, .. }
        | ContextAction::Mem { tree_hash, .. }
        | ContextAction::DirMem { tree_hash, .. }
        | ContextAction::Set { tree_hash, .. }
        | ContextAction::Copy { tree_hash, .. }
        | ContextAction::Delete { tree_hash, .. }
        | ContextAction::RemoveRecursively { tree_hash, .. }
        | ContextAction::Commit { tree_hash, .. }
        | ContextAction::Fold { tree_hash, .. } => {
            Some(MerkleHash::try_from(tree_hash.as_slice()).unwrap())
        }
        ContextAction::Checkout { .. } | ContextAction::Shutdown => None,
    }
}

fn parse_mem_value(value: &str) -> usize {
    let mut pair = value.split_whitespace();
    let value: usize = pair.next().unwrap().parse().unwrap();

    match pair.next() {
        Some("kB") => value * 1024,
        _ => panic!("unknown memory usage unit."),
    }
}

#[cfg(target_os = "linux")]
fn mem_usage() -> usize {
    let mut text_buf = Default::default();

    File::open("/proc/self/status")
        .and_then(|mut f| f.read_to_string(&mut text_buf))
        .unwrap();

    let mut res = 0;

    for line in text_buf.lines() {
        let mut pairs = line.split(':');
        match (pairs.next(), pairs.next()) {
            // (Some("VmPeak"), Some(text)) => virt_peak = parse_mem_value(text),
            // (Some("VmSize"), Some(text)) => virt = parse_mem_value(text),
            (Some("VmRSS"), Some(text)) => res = parse_mem_value(text),
            // (Some("VmSwap"), Some(text))
            _ => {}
        }
    }

    res
}

#[cfg(not(target_os = "linux"))]
fn mem_usage() -> usize {
    0
}

struct Args {
    backend: String,
    test_integrity: bool,
    preserved_cycles: usize,
    cycle_block_count: u32,
    actions_file: String,
}

impl Args {
    pub fn read_args() -> Self {
        let app = App::new("merkle_storage_stats")
            .about("generate merkle storage statistics data")
            .arg(Arg::with_name("preserved_cycles")
                 .long("preserved-cycles")
                 .help("last number of cycles which should be preserved")
                 .default_value("7"))
            .arg(Arg::with_name("cycle_block_count")
                 .long("cycle-block-count")
                 .help("amount of blocks in each cycle")
                 .default_value("2048"))
            .arg(Arg::with_name("test_integrity")
                 .takes_value(false)
                 .long("test-integrity")
                 .help("validate integrity of all commits for last 7 cycles"))
            .arg(Arg::with_name("actions_file")
                 .required(true)
                 .help("path to the actions.bin")
                 .index(1))
            .arg(Arg::with_name("backend")
                 .short("b")
                 .long("backend")
                 .default_value("in-memory-gced")
                 .help("backend to use for storing merkle storage. Possible values: in-memory-gced, in-memory-mark-sweep-gced")
                 // TODO add support and test with other backends ("rocksdb, sled, in-memory-gced, in-memory-mark-sweep-gced")
                 );

        let matches = app.get_matches();

        Self {
            backend: matches
                .value_of("backend")
                .unwrap_or("in-memory-gced")
                .to_string(),
            test_integrity: matches.is_present("test_integrity"),
            preserved_cycles: matches
                .value_of("preserved_cycles")
                .unwrap_or("7")
                .parse()
                .unwrap(),
            cycle_block_count: matches
                .value_of("cycle_block_count")
                .unwrap_or("2048")
                .parse()
                .unwrap(),
            actions_file: matches
                .value_of("actions_file")
                .expect("actions_file is required argument")
                .to_string(),
        }
    }
}

fn main() {
    gen_stats(Args::read_args());
}

fn gen_stats(args: Args) {
    let mut cycle_commit_hashes: Vec<Vec<EntryHash>> =
        vec![Default::default(); args.preserved_cycles - 1];

    let backend: Box<dyn StorageBackend + Send + Sync> = match args.backend.as_str() {
        "in-memory-gced" => Box::new(KVStoreGCed::<BTreeMapBackend>::new(args.preserved_cycles)),
        "in-memory-mark-sweep-gced" => {
            Box::new(MarkSweepGCed::<InMemoryBackend>::new(args.preserved_cycles))
        }
        _ => {
            eprintln!("unsupported backend supplied: {}", args.backend);
            return;
        }
    };

    let mut merkle = MerkleStorage::new(backend);

    if !args.test_integrity {
        println!("block level, key bytes, value bytes, reused keys bytes, total mem, process mem, total latency");
    }
    let mut level = 0;
    for actions in ActionsFileReader::new(&args.actions_file)
        .unwrap()
        .into_iter()
    {
        level += 1;
        for action in actions.into_iter() {
            if args.test_integrity {
                if let ContextAction::Commit {
                    new_context_hash, ..
                } = &action.action
                {
                    cycle_commit_hashes
                        .last_mut()
                        .unwrap()
                        .push(new_context_hash[..].try_into().unwrap());
                }
            }

            if let Some(hash) = &get_tree_hash(&action.action) {
                merkle.stage_checkout(hash).unwrap();
            }

            match &action.action {
                ContextAction::Set { key, value, .. } => {
                    merkle.set(&key, &value).unwrap();
                }
                ContextAction::Copy {
                    to_key, from_key, ..
                } => {
                    merkle.copy(&from_key, &to_key).unwrap();
                }
                ContextAction::Delete { key, .. } => {
                    merkle.delete(&key).unwrap();
                }
                ContextAction::RemoveRecursively { key, .. } => {
                    merkle.delete(&key).unwrap();
                }
                ContextAction::Commit {
                    author,
                    message,
                    date,
                    ..
                } => {
                    merkle
                        .commit(*date as u64, author.to_string(), message.to_string())
                        .unwrap();
                }
                ContextAction::Checkout { context_hash, .. } => {
                    merkle
                        .checkout(&context_hash[..].try_into().unwrap())
                        .unwrap();
                }
                _ => {}
            };
        }

        if !args.test_integrity {
            let stats = merkle.get_merkle_stats();
            println!(
                "{}, {}, {}, {}, {}, {}, {}",
                level,
                stats.kv_store_stats.key_bytes,
                stats.kv_store_stats.value_bytes,
                stats.kv_store_stats.reused_keys_bytes,
                stats.kv_store_stats.total_as_bytes(),
                mem_usage(),
                merkle.get_block_latency(0).unwrap(),
            );
        }

        let cycle = level / args.cycle_block_count;

        if level % args.cycle_block_count == 0 && level > 0 {
            merkle.start_new_cycle().unwrap();

            if args.test_integrity {
                let commits_iter = cycle_commit_hashes.iter().flatten().cloned();
                check_commit_hashes(&merkle, commits_iter).unwrap();

                cycle_commit_hashes = cycle_commit_hashes
                    .into_iter()
                    .skip(1)
                    .chain(vec![vec![]])
                    .collect();
                println!("cycle #{} - integrity intact!", cycle);
            }
        }
    }
}
