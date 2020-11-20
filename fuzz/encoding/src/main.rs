// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use rand::{distributions::Alphanumeric, prelude::*, seq::SliceRandom};
use std::iter;
use tezos_encoding::binary_reader::BinaryReader;
use tezos_encoding::encoding::{Encoding, Field};

use honggfuzz::fuzz;
use log::debug;

fn main() {
    loop {
        const ENCODING_LIFETIME: usize = 100_000;
        for _ in 0..ENCODING_LIFETIME {
            let encoding = generate_random_encoding();
            fuzz!(|data: &[u8]| {
                if let Err(e) = BinaryReader::new().read(data, &encoding) {
                    debug!(
                        "BinaryReader produced error for input: {:?}\nError:\n{:?}",
                        data, e
                    );
                }
            });
        }
    }
}

fn generate_random_encoding() -> Encoding {
    // 1. start with tuple or object
    let is_tuple: bool = random();
    if is_tuple {
        let mut fields = Vec::new();
        for _ in 0..10 {
            fields.push(gen_random_member());
        }
        Encoding::Tup(fields)
    } else {
        let mut fields = Vec::new();
        for _ in 0..10 {
            fields.push(gen_random_field());
        }
        Encoding::Obj(fields)
    }
}

fn gen_random_member() -> Encoding {
    use Encoding::*;
    let mut rng = rand::thread_rng();

    let encodings = [
        Unit,
        Int8,
        Uint8,
        Int16,
        Uint16,
        Int31,
        Int32,
        Uint32,
        Int64,
        RangedInt,
        Z,
        Mutez,
        Float,
        RangedFloat,
        Bool,
        String,
        Bytes,
        Timestamp,
        // TODO: Add implement for complex sub-types
    ];

    encodings.choose(&mut rng).unwrap().clone()
}

fn gen_random_field() -> Field {
    Field::new(&gen_random_name(), gen_random_member())
}

fn gen_random_name() -> String {
    let mut rng = rand::thread_rng();
    iter::repeat(())
        .map(|_| rng.sample(Alphanumeric))
        .take(15)
        .collect()
}
