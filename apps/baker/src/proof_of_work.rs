// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryInto;

use crypto::blake2b;
use tezos_encoding::enc::BinWriter;
use tezos_messages::protocol::proto_012::operation::FullHeader;

pub fn guess_proof_of_work(header: &FullHeader, proof_of_work_threshold: u64) -> [u8; 8] {
    let mut header_bytes = vec![];
    header.bin_write(&mut header_bytes).unwrap();
    let fitness_size = u32::from_be_bytes(header_bytes[78..82].try_into().unwrap()) as usize;
    let nonce_pos = fitness_size + 150;

    loop {
        let nonce = header_bytes[nonce_pos..(nonce_pos + 8)].try_into().unwrap();
        if check_proof_of_work(&header_bytes, proof_of_work_threshold) {
            break nonce;
        }

        let nonce = u64::from_be_bytes(nonce).wrapping_add(1).to_be_bytes();
        header_bytes[nonce_pos..(nonce_pos + 8)].clone_from_slice(&nonce);
    }
}

fn check_proof_of_work(header_bytes: &[u8], proof_of_work_threshold: u64) -> bool {
    let hash = blake2b::digest_256(&header_bytes).unwrap();
    let stamp = u64::from_be_bytes(hash[0..8].try_into().unwrap());
    stamp < proof_of_work_threshold
}

#[cfg(test)]
mod tests {
    use crypto::hash::{
        BlockHash, BlockPayloadHash, ContextHash, OperationListListHash, Signature,
    };
    use tezos_encoding::enc::BinWriter;
    use tezos_messages::protocol::proto_012::operation::FullHeader;

    use crate::proof_of_work::check_proof_of_work;

    #[test]
    fn pow_test() {
        let proof_of_work_threshold = 70368744177663_u64;
        let header = FullHeader {
            level: 232680,
            proto: 2,
            predecessor: BlockHash::from_base58_check(
                "BLu68WtjmwxgPoogFbMXCY1P8gkebaXdBDd1TFAsYjz3vZNyyLv",
            )
            .unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2022-03-14T10:02:35Z")
                .unwrap()
                .timestamp(),
            validation_pass: 4,
            operations_hash: OperationListListHash::from_base58_check(
                "LLoZMEAWjpyMPz19PKzYv2Zbs3kyFDe8XpDzj45wa998ZkCruePZo",
            )
            .unwrap(),
            fitness: vec![
                vec![0x02],
                vec![0x00, 0x03, 0x8c, 0xe8],
                vec![],
                vec![0xff, 0xff, 0xff, 0xff],
                vec![0x00, 0x00, 0x00, 0x00],
            ],
            context: ContextHash::from_base58_check(
                "CoVmcqcynAhio4fodmyNgAcJGKNoyCHPygdBhKGredvUQSjTappc",
            )
            .unwrap(),
            payload_hash: BlockPayloadHash::from_base58_check(
                "vh1mi89F7NNTDQGWLoyhccPzSsuN5RLMAB1EsPdJ9zHZ39XZx39v",
            )
            .unwrap(),
            payload_round: 0,
            proof_of_work_nonce: hex::decode("409a3f3ff9820000").unwrap(),
            liquidity_baking_escape_vote: false,
            seed_nonce_hash: None,
            signature: Signature(vec![0x00; 64]),
        };
        let mut header_bytes = vec![];
        header.bin_write(&mut header_bytes).unwrap();
        assert!(check_proof_of_work(&header_bytes, proof_of_work_threshold));
    }

    #[test]
    fn pow_fail_test() {
        let proof_of_work_threshold = 70368744177663_u64;
        let header = FullHeader {
            level: 234487,
            proto: 2,
            predecessor: BlockHash::from_base58_check(
                "BKnspHSGAkdjjRQ5k83tPzx73UueJLHmW1jgT4fwTrMyoXUPT2o",
            )
            .unwrap(),
            timestamp: 1647280025,
            validation_pass: 4,
            operations_hash: OperationListListHash::from_base58_check(
                "LLoa6FrQ37toJPdS2PU1cepexNrvncEyqj8D4KtYf1qpccSu1NxSH",
            )
            .unwrap(),
            fitness: vec![
                vec![0x02],
                234487u32.to_be_bytes().to_vec(),
                vec![],
                vec![255, 255, 255, 255],
                vec![0, 0, 0, 0],
            ],
            context: ContextHash::from_base58_check(
                "CoUjRZrB2TVagSRRYJJcc3Y7SUTfSLqZc55EktpYPpfp8xnR6PM8",
            )
            .unwrap(),
            payload_hash: BlockPayloadHash::from_base58_check(
                "vh2VjzHiqaTcKh8wu5AbP6TZ5ibRCqkz7qQ2bAhGK7i2Ffj2akkp",
            )
            .unwrap(),
            payload_round: 0,
            proof_of_work_nonce: vec![121, 133, 250, 254, 31, 183, 3, 1],
            liquidity_baking_escape_vote: false,
            seed_nonce_hash: None,
            signature: Signature(vec![0x00; 64]),
        };
        let mut header_bytes = vec![];
        header.bin_write(&mut header_bytes).unwrap();
        assert!(!check_proof_of_work(&header_bytes, proof_of_work_threshold));
    }
}
