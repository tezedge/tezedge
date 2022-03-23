// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fs::File, path::PathBuf, io::{self, Read}, collections::BTreeMap, convert::TryInto};

use derive_more::From;
use tezos_encoding::types::SizedBytes;
use thiserror::Error;

use crypto::{hash::NonceHash, blake2b};
use tezos_messages::protocol::proto_005_2::operation::SeedNonceRevelationOperation;

pub struct SeedNonceService {
    file_path: PathBuf,
    seeds: BTreeMap<u32, Vec<u8>>,
    blocks_per_commitment: u32,
    blocks_per_cycle: u32,
    nonce_length: usize,
}

#[derive(Debug, Error, From)]
pub enum SeedPersistanceError {
    #[error("{_0}")]
    Io(io::Error),
    #[error("{_0}")]
    Serde(serde_json::Error),
    #[error("has no seed for level: {_0}")]
    NoSeedFor(i32),
}

impl SeedNonceService {
    pub fn new(
        base_dir: &PathBuf,
        baker: &str,
        blocks_per_commitment: u32,
        blocks_per_cycle: u32,
        nonce_length: usize,
    ) -> Result<Self, SeedPersistanceError> {
        let file_path = base_dir.join(format!("seed_nonce_{baker}.json"));
        let mut s = String::new();
        if file_path.is_file() {
            File::open(&file_path)?.read_to_string(&mut s)?;
        }
        let seeds = if !s.is_empty() {
            serde_json::from_str(&s)?
        } else {
            BTreeMap::default()
        };
        Ok(SeedNonceService {
            file_path,
            seeds,
            blocks_per_commitment,
            blocks_per_cycle,
            nonce_length,
        })
    }

    pub fn gen_nonce(
        &mut self,
        level: i32,
    ) -> Result<Option<NonceHash>, SeedPersistanceError> {
        let level = level as u32;
        if level % self.blocks_per_commitment == 0 {
            let vec = (0..self.nonce_length)
                .map(|_| rand::random())
                .collect::<Vec<u8>>();
            let hash = NonceHash(blake2b::digest_256(&vec).unwrap());
            self.seeds.insert(level, vec);
            serde_json::to_writer(File::create(&self.file_path)?, &self.seeds)?;
            Ok(Some(hash))
        } else {
            Ok(None)
        }
    }

    pub fn reveal_nonce(
        &mut self,
        level: i32,
    ) -> Option<impl Iterator<Item = SeedNonceRevelationOperation> + '_> {
        let level = level as u32;

        let range = if level % self.blocks_per_cycle == 0 {
            if level < self.blocks_per_cycle {
                return None;
            }
            (level - self.blocks_per_cycle)..level
        } else {
            return None;
        };
        if range.is_empty() {
            None
        } else {
            let it = self
                .seeds
                .range(range)
                .map(|(level, nonce)| SeedNonceRevelationOperation {
                    level: *level as i32,
                    nonce: SizedBytes(nonce.as_slice().try_into().unwrap()),
                });
            Some(it)
        }
    }
}
