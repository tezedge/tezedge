// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read},
    path::PathBuf, mem, convert::TryInto,
};

use derive_more::From;
use tezos_encoding::types::SizedBytes;
use thiserror::Error;
use serde::{Serialize, Deserialize};

use crypto::{blake2b, hash::NonceHash};
use tezos_messages::protocol::{proto_005_2::operation::SeedNonceRevelationOperation, proto_012::operation::Contents};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Nonce(Vec<u8>);

impl Nonce {
    pub fn random(length: usize) -> Self {
        Nonce((0..length).map(|_| rand::random()).collect())
    }
}

impl serde::Serialize for Nonce {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            hex::encode(&self.0).serialize(serializer)
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> serde::Deserialize<'de> for Nonce {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            hex::decode(String::deserialize(deserializer)?)
                .map_err(serde::de::Error::custom)
                .map(Nonce)
        } else {
            Vec::deserialize(deserializer)
                .map(Nonce)
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct CycleNonces {
    cycle: u32,
    // map from nonce into position in cycle
    // previous cycle: (cycle - 1)
    previous: BTreeMap<Nonce, u32>,
    // this cycle
    this: BTreeMap<Nonce, u32>,
}

impl CycleNonces {
    pub fn insert(&mut self, cycle: u32, pos: u32, nonce: Nonce) {
        if cycle == self.cycle + 1 {
            self.cycle = cycle;
            self.previous = mem::take(&mut self.this);
        }
        if cycle > self.cycle + 1 {
            self.previous.clear();
        }
        self.this.insert(nonce, pos);
    }

    pub fn take(&mut self, cycle: u32) -> BTreeMap<Nonce, u32> {
        if cycle == self.cycle {
            mem::take(&mut self.previous)
        } else {
            BTreeMap::default()
        }
    }
}

pub struct SeedNonceService {
    file_path: PathBuf,
    nonces: CycleNonces,
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
        let nonces = if !s.is_empty() {
            serde_json::from_str(&s)?
        } else {
            CycleNonces::default()
        };
        Ok(SeedNonceService {
            file_path,
            nonces,
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
            let nonce = Nonce::random(self.nonce_length);
            let hash = NonceHash(blake2b::digest_256(&nonce.0).unwrap());
            self.nonces.insert(level / self.blocks_per_cycle, level % self.blocks_per_cycle, nonce);
            serde_json::to_writer(File::create(&self.file_path)?, &self.nonces)?;
            Ok(Some(hash))
        } else {
            Ok(None)
        }
    }

    pub fn reveal_nonce(
        &mut self,
        level: i32,
    ) -> impl Iterator<Item = Contents> + '_ {
        let level = level as u32;

        let cycle = level / self.blocks_per_cycle;
        self.nonces.take(cycle).into_iter()
            .map(move |(Nonce(nonce), pos)| {
                Contents::SeedNonceRevelation(SeedNonceRevelationOperation {
                    level: ((cycle - 1) * self.blocks_per_cycle + pos) as i32,
                    nonce: SizedBytes(nonce.as_slice().try_into().unwrap()),
                })
            })
            .into_iter()
    }
}
