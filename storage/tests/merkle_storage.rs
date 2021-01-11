// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryInto;
use std::error::Error;
use std::collections::VecDeque;
use serde::{Serialize, Deserialize};
use crypto::hash::HashType;

use storage::{
    in_memory::KVStore,
    context_action_storage::ContextAction,
    merkle_storage::{
        MerkleStorage,
        MerkleError,
        Entry,
        check_entry_hash,
    },
};

type GetBlocksResponse = Vec<GetBlockResponse>;

#[derive(Serialize, Deserialize)]
struct GetBlockResponse {
    hash: String,
    metadata: GetBlockResponseMetadata,
}

#[derive(Serialize, Deserialize)]
struct GetBlockResponseMetadata {
    level: GetBlockResponseMetadataLevel,
}

#[derive(Serialize, Deserialize)]
struct GetBlockResponseMetadataLevel {
    cycle: usize,
    cycle_position: usize,
}

type GetActionsResponse = Vec<GetActionResponse>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetActionResponse {
    pub action: ContextAction,
}

struct TezEdgeApi {
    client: reqwest::blocking::Client,
}

impl TezEdgeApi {
    pub const NODE_URL: &'static str = "http://master.dev.tezedge.com:18732";

    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
        }
    }

    pub fn get_blocks_url(limit: usize, offset: usize) -> String {
        format!(
            "{}/dev/chains/main/blocks?limit={}&from_block_id={}",
            Self::NODE_URL,
            limit,
            offset.max(1),
        )
    }

    pub fn get_block_actions_url<H>(block_hash: H) -> String
    where H: AsRef<str> {
        format!(
            "{}/dev/chains/main/actions/blocks/{}",
            Self::NODE_URL,
            block_hash.as_ref(),
        )
    }

    pub fn get_blocks(&self, limit: usize, offset: usize) -> Result<GetBlocksResponse, Box<dyn Error>> {
        Ok(self.client.get(&Self::get_blocks_url(limit, offset))
            .send()?
            .json::<_>()?)
    }

    pub fn get_block_actions<H>(&self, block_hash: H) -> Result<GetActionsResponse, Box<dyn Error>>
    where H: AsRef<str> {
        Ok(self.client.get(&Self::get_block_actions_url(block_hash))
            .send()?
            .json::<_>()?)
    }
}


#[test]
fn test_merkle_storage_gc() {
    let limit = 128;
    let cycle = 4096;
    let api = TezEdgeApi::new();
    let mut merkle = MerkleStorage::new(Box::new(storage::in_memory::KVStore::new()));

    let blocks_iter = (1..).step_by(limit).flat_map(|offset| {
        api.get_blocks(limit, offset).unwrap().into_iter()
    });

    let (mut prev_cycle_commits, mut commits) = (vec![], vec![]);

    for (index, block) in blocks_iter.enumerate() {
        let actions_iter = api.get_block_actions(block.hash).unwrap()
            .into_iter()
            .map(|x| x.action);

        for action in actions_iter {
            if let ContextAction::Commit { new_context_hash, .. } = &action {
                commits.push(new_context_hash[..].try_into().unwrap());
            }
            merkle.apply_context_action(&action).unwrap();
        }

        let (cycle, cycle_position) = (
            block.metadata.level.cycle,
            block.metadata.level.cycle_position,
        );

        if cycle != 0 && cycle_position == 0 && prev_cycle_commits.len() > 0 {
            for commit_hash in prev_cycle_commits.into_iter() {
                merkle.gc_commit(&commit_hash);
            }

            for commit_hash in commits.iter() {
                assert!(matches!(check_entry_hash(&merkle, commit_hash), Ok(_)));
            }

            prev_cycle_commits = commits;
            commits = vec![];
        }
    }
}
