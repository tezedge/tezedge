// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeMap, mem};

use crypto::{blake2b, hash::NonceHash};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct Nonce(pub Vec<u8>);

impl Serialize for Nonce {
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

impl<'de> Deserialize<'de> for Nonce {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            hex::decode(String::deserialize(deserializer)?)
                .map_err(serde::de::Error::custom)
                .map(Nonce)
        } else {
            Vec::deserialize(deserializer).map(Nonce)
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct CycleNonce {
    // immutable config
    pub blocks_per_commitment: u32,
    pub blocks_per_cycle: u32,
    pub nonce_length: usize,
    // this cycle
    pub cycle: u32,
    // map from nonce into position in cycle
    // previous cycle: (cycle - 1)
    pub previous: BTreeMap<Nonce, u32>,
    // this cycle
    pub this: BTreeMap<Nonce, u32>,
}

impl CycleNonce {
    pub fn gen_nonce(&mut self, level: i32) -> Option<NonceHash> {
        let level = level as u32;
        if level % self.blocks_per_commitment == 0 {
            // TODO: this is effect
            let nonce = Nonce((0..self.nonce_length).map(|_| rand::random()).collect());
            let hash = NonceHash(blake2b::digest_256(&nonce.0).unwrap());

            let pos = level % self.blocks_per_cycle;
            self.this.insert(nonce, pos);

            Some(hash)
        } else {
            None
        }
    }

    pub fn reveal_nonce(&mut self, level: i32) -> impl Iterator<Item = (i32, Vec<u8>)> + '_ {
        let level = level as u32;

        let cycle = level / self.blocks_per_cycle;
        if cycle == self.cycle + 1 {
            self.cycle = cycle;
            self.previous = mem::take(&mut self.this);
        }
        if cycle > self.cycle + 1 {
            self.cycle = cycle;
            self.previous.clear();
            self.this.clear();
        }

        let previous = if cycle == self.cycle {
            mem::take(&mut self.previous)
        } else {
            BTreeMap::default()
        };

        previous.into_iter().map(move |(Nonce(nonce), pos)| {
            let level = ((cycle - 1) * self.blocks_per_cycle + pos) as i32;
            (level, nonce)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::CycleNonce;

    #[test]
    fn cycle_nonce_seed_reveal() {
        let mut state = CycleNonce {
            blocks_per_commitment: 4,
            blocks_per_cycle: 4096,
            nonce_length: 32,
            cycle: 0,
            previous: BTreeMap::new(),
            this: BTreeMap::new(),
        };

        let mut c = 0;
        let l = state.blocks_per_cycle as i32;

        // should have seeds
        let mut n = 0;

        // simulate one cycle
        for level in (c * l)..((c + 1) * l) {
            let reveal = state.reveal_nonce(level).collect::<Vec<_>>();
            assert!(reveal.is_empty());

            // 1/8 chance
            if rand::random::<u8>() < (256 / 8) as u8 {
                state.gen_nonce(level);
                if (level as u32) % state.blocks_per_commitment == 0 {
                    n += 1;
                }
            }
        }

        // simulate next cycle
        c += 1;
        let mut revealed = vec![];
        for level in (c * l)..((c + 1) * l) {
            let mut reveal = state.reveal_nonce(level).collect::<Vec<_>>();
            revealed.append(&mut reveal);
        }

        assert_eq!(revealed.len(), n);
        println!("{n}");
    }
}
