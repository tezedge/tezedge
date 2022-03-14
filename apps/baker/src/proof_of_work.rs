// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT


#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use crypto::hash::{BlockHash, OperationListListHash, ContextHash, BlockPayloadHash, Signature};
    use tezos_encoding::enc::BinWriter;
    use tezos_messages::{
        p2p::{encoding::block_header::BlockHeader, binary_message::{BinaryRead, MessageHash}},
        protocol::proto_012::operation::FullHeader,
    };

    #[test]
    fn pow_test() {
        let proof_of_work_threshold = 70368744177663_i64;
        let header = FullHeader {
            level: 232680,
            proto: 2,
            predecessor: BlockHash::from_base58_check("BLu68WtjmwxgPoogFbMXCY1P8gkebaXdBDd1TFAsYjz3vZNyyLv").unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2022-03-14T10:02:35Z").unwrap().timestamp(),
            validation_pass: 4,
            operations_hash: OperationListListHash::from_base58_check("LLoZMEAWjpyMPz19PKzYv2Zbs3kyFDe8XpDzj45wa998ZkCruePZo").unwrap(),
            fitness: vec![vec![0x02], vec![0x00, 0x03, 0x8c, 0xe8], vec![], vec![0xff, 0xff, 0xff, 0xff], vec![0x00, 0x00, 0x00, 0x00]],
            context: ContextHash::from_base58_check("CoVmcqcynAhio4fodmyNgAcJGKNoyCHPygdBhKGredvUQSjTappc").unwrap(),
            payload_hash: BlockPayloadHash::from_base58_check("vh1mi89F7NNTDQGWLoyhccPzSsuN5RLMAB1EsPdJ9zHZ39XZx39v").unwrap(),
            payload_round: 0,
            proof_of_work_nonce: hex::decode("409a3f3ff9820000").unwrap(),
            liquidity_baking_escape_vote: false,
            seed_nonce_hash: None,
            signature: Signature(vec![0x00; 64]),
        };
        let mut header_bytes = vec![];
        header.bin_write(&mut header_bytes).unwrap();
        let block_header = BlockHeader::from_bytes(&header_bytes).unwrap();
        let block_hash = block_header.message_typed_hash::<BlockHash>().unwrap();
        let stamp = i64::from_be_bytes(block_hash.0[0..8].try_into().unwrap());
        assert!(stamp < proof_of_work_threshold);
    }
}
