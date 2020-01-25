// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use storage::num_from_slice;
use crypto::blake2b;
use failure::Fail;

macro_rules! merge_slices {
    ( $($x:expr),* ) => {{
        let mut res = vec![];
        $(
            res.extend_from_slice($x);
        )*
        res
    }}
}

// cycle in which is given level
// level 0 (genesis block) is not part of any cycle (cycle 0 starts at level 1), hence the -1
pub fn cycle_from_level(level: i32, blocks_per_cycle: i32) -> i32 {
    if blocks_per_cycle == 0 {
        (level - 1) / 2048
    } else {
        (level - 1) / blocks_per_cycle
    }
}

// the position of the block in its cycle
// level 0 (genesis block) is not part of any cycle (cycle 0 starts at level 1)
// hence the blocks_per_cycle - 1 for last cycle block
pub fn level_position(level:i32, blocks_per_cycle:i32) -> i32 {
    // set defaut blocks_per_cycle in case that 0 is given as parameter
    let blocks_per_cycle = if blocks_per_cycle == 0 {
        2048
    } else {
        blocks_per_cycle
    };
    let cycle_position = (level % blocks_per_cycle) - 1;
    if cycle_position < 0 { //for last block
        blocks_per_cycle - 1
    } else {
        cycle_position
    }
}

#[derive(Debug, Fail)]
pub enum TezosPRNGError {
    #[fail(display = "Value of bound(last_roll) not correct: {} bytes", bound)]
    BoundNotCorrect {
        bound: i32
    },
}

type RandomSeedState = Vec<u8>;
pub type TezosPRNGResult = Result<(i32, RandomSeedState), TezosPRNGError>;

// tezos PRNG
// input: 
// state: RandomSeedState, initially the random seed
// nonce_size: nonce_length from current protocol constants
// blocks_per_cycle: blocks_per_cycle from current protocol constants
// use_string_bytes: string converted to bytes, i.e. endorsing rights use b"level endorsement:"
// level: block level
// offset: for baking priority, for endorsing slot
// bound: last possible roll nuber that have meaning to be generated (last_roll from context list)
// output: pseudo random generated roll number and RandomSeedState for next roll generation if the roll provided is missing from the roll list
pub fn get_pseudo_random_number(state: RandomSeedState, nonce_size: usize, blocks_per_cycle: i32, use_string_bytes: &[u8], level: i32, offset: i32, bound: i32) -> TezosPRNGResult {
    if bound < 1 {
        return Err(TezosPRNGError::BoundNotCorrect{bound: bound})
    }
    // nonce_size == nonce_hash_size == 32 in the current protocol
    let zero_bytes: Vec<u8> = vec![0; nonce_size];

    // the position of the block in its cycle
    let cycle_position = level_position(level, blocks_per_cycle);

    // take the state (initially the random seed), zero bytes, the use string and the blocks position in the cycle as bytes, merge them together and hash the result
    let rd = blake2b::digest_256(&merge_slices!(&state, &zero_bytes, use_string_bytes, &cycle_position.to_be_bytes())).to_vec();

    // take the 4 highest bytes and xor them with the priority/slot (offset)
    let higher = num_from_slice!(rd, 0, i32) ^ offset;

    // set the 4 highest bytes to the result of the xor operation
    let mut sequence = blake2b::digest_256(&merge_slices!(&higher.to_be_bytes(), &rd[4..])).to_vec();
    let v: i32;
    // Note: this part aims to be similar 
    // hash once again and take the 4 highest bytes and we got our random number
    loop {
        let hashed = blake2b::digest_256(&sequence).to_vec();

        // computation for overflow check
        let drop_if_over = i32::max_value() - (i32::max_value() % bound);

        // 4 highest bytes
        let r = num_from_slice!(hashed, 0, i32).abs();

        // potentional overflow, keep the state of the generator and do one more iteration
        sequence = hashed;
        if r >= drop_if_over {
            continue;
        // use the remainder(mod) operation to get a number from a desired interval
        } else {
            v = r % bound;
            break;
        };
    }
    Ok((v.into(), sequence))
}

