// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryInto;
use crypto::hash::{HashType, BlockHash};
use clap::{Arg, App};

use storage::*;
use context_action_storage::ContextAction;
use merkle_storage::{MerkleStorage, Entry, EntryHash, check_commit_hashes};

mod actions_tool;
use actions_tool::ActionsFileReader;

struct Args {
    preserved_cycles: usize,
    cycle_block_count: u32,
    actions_file: String,
}

impl Args {
    pub fn read_args() -> Self {
        let app = App::new("merkle_storage_stats")
            .about("generate merkle storage statistics data")
            .arg(Arg::with_name("preserved_cycles")
                 .help("last number of cycles which should be preserved")
                 .default_value("5"))
            .arg(Arg::with_name("cycle_block_count")
                 .help("amount of blocks in each cycle")
                 .default_value("2048"))
            .arg(Arg::with_name("actions_file")
                 .required(true)
                 .help("path to the actions.bin")
                 .index(1));

        let matches = app.get_matches();

        Self {
            preserved_cycles: matches.value_of("preserved_cycles")
                .unwrap_or("5")
                .parse()
                .unwrap(),
            cycle_block_count: matches.value_of("cycle_block_count")
                .unwrap_or("2048")
                .parse()
                .unwrap(),
            actions_file: matches.value_of("actions_file")
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

    let mut merkle = MerkleStorage::new();

    println!("block level, key bytes, value bytes, reused keys bytes, total mem, total latency");

    for (block, actions) in ActionsFileReader::new(&args.actions_file).unwrap().into_iter() {
        let actions_len = actions.len();

        for action in actions.into_iter() {
            if let ContextAction::Commit { new_context_hash, .. } = &action {
                cycle_commit_hashes.last_mut().unwrap().push(
                    new_context_hash[..].try_into().unwrap()
                );
            }
            merkle.apply_context_action(&action).unwrap();
        }

        let stats = merkle.get_merkle_stats().unwrap();
        println!("{}, {}, {}, {}, {}, {}",
            block.block_level,
            stats.kv_store_stats.key_bytes,
            stats.kv_store_stats.value_bytes,
            stats.kv_store_stats.reused_keys_bytes,
            stats.kv_store_stats.total_as_bytes(),
            merkle.get_block_latency(0).unwrap(),
        );

        let level = block.block_level;

        if level % args.cycle_block_count == 0 && level > 0 {
            merkle.start_new_cycle().unwrap();

            let commits_iter = cycle_commit_hashes.iter()
                .flatten()
                .cloned();
            check_commit_hashes(&merkle, commits_iter).unwrap();

            cycle_commit_hashes = cycle_commit_hashes.into_iter()
                .skip(1)
                .chain(vec![vec![]])
                .collect();
        }
    }
}
