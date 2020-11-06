// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};

use tezos_encoding::{
    encoding::{Encoding, Field, HasEncoding},
    has_encoding,
    types::BigInt,
};

use crate::non_cached_data;
use crate::protocol::{ToRpcJsonMap, UniversalValue};

pub const FIXED: FixedConstants = FixedConstants {
    proof_of_work_nonce_size: 8,
    nonce_length: 32,
    max_anon_ops_per_block: 132,
    max_operation_data_length: 16 * 1024,
    max_proposals_per_delegate: 20,
};

#[derive(Serialize, Deserialize, Debug, Clone, CopyGetters)]
pub struct FixedConstants {
    proof_of_work_nonce_size: u8,
    #[get_copy = "pub"]
    nonce_length: u8,
    max_anon_ops_per_block: u8,
    max_operation_data_length: i32,
    max_proposals_per_delegate: u8,
}

impl ToRpcJsonMap for FixedConstants {
    fn as_map(&self) -> HashMap<&'static str, UniversalValue> {
        let mut ret: HashMap<&'static str, UniversalValue> = Default::default();
        ret.insert("proof_of_work_nonce_size", UniversalValue::num(self.proof_of_work_nonce_size));
        ret.insert("nonce_length", UniversalValue::num(self.nonce_length));
        ret.insert("max_anon_ops_per_block", UniversalValue::num(self.max_anon_ops_per_block));
        ret.insert("max_operation_data_length", UniversalValue::num(self.max_operation_data_length));
        ret.insert("max_proposals_per_delegate", UniversalValue::num(self.max_proposals_per_delegate));
        ret
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone, Getters, CopyGetters)]
pub struct ParametricConstants {
    #[get_copy = "pub"]
    preserved_cycles: u8,
    #[get_copy = "pub"]
    blocks_per_cycle: i32,
    blocks_per_commitment: i32,
    #[get_copy = "pub"]
    blocks_per_roll_snapshot: i32,
    blocks_per_voting_period: i32,
    #[get = "pub"]
    time_between_blocks: Vec<i64>,
    #[get_copy = "pub"]
    endorsers_per_block: u16,
    hard_gas_limit_per_operation: BigInt,
    hard_gas_limit_per_block: BigInt,
    proof_of_work_threshold: i64,
    tokens_per_roll: BigInt,
    michelson_maximum_type_size: u16,
    seed_nonce_revelation_tip: BigInt,
    origination_size: i32,
    block_security_deposit: BigInt,
    endorsement_security_deposit: BigInt,
    baking_reward_per_endorsement: Vec<BigInt>,
    endorsement_reward: Vec<BigInt>,
    cost_per_byte: BigInt,
    hard_storage_limit_per_operation: BigInt,
    test_chain_duration: i64,
    quorum_min: i32,
    quorum_max: i32,
    min_proposal_quorum: i32,
    initial_endorsers: u16,
    delay_per_missing_endorsement: i64,
}

non_cached_data!(ParametricConstants);
has_encoding!(ParametricConstants, PARAMETRIC_CONSTANTS_ENCODING, {
        Encoding::Obj(vec![
            Field::new("preserved_cycles", Encoding::Uint8),
            Field::new("blocks_per_cycle", Encoding::Int32),
            Field::new("blocks_per_commitment", Encoding::Int32),
            Field::new("blocks_per_roll_snapshot", Encoding::Int32),
            Field::new("blocks_per_voting_period", Encoding::Int32),
            Field::new("time_between_blocks", Encoding::dynamic(Encoding::list(Encoding::Int64))),
            Field::new("endorsers_per_block", Encoding::Uint16),
            Field::new("hard_gas_limit_per_operation", Encoding::Z),
            Field::new("hard_gas_limit_per_block", Encoding::Z),
            Field::new("proof_of_work_threshold", Encoding::Int64),
            Field::new("tokens_per_roll", Encoding::Mutez),
            Field::new("michelson_maximum_type_size", Encoding::Uint16),
            Field::new("seed_nonce_revelation_tip", Encoding::Mutez),
            Field::new("origination_size", Encoding::Int32),
            Field::new("block_security_deposit", Encoding::Mutez),
            Field::new("endorsement_security_deposit", Encoding::Mutez),
            Field::new("baking_reward_per_endorsement", Encoding::dynamic(Encoding::list(Encoding::Mutez))),
            Field::new("endorsement_reward", Encoding::dynamic(Encoding::list(Encoding::Mutez))),
            Field::new("cost_per_byte", Encoding::Mutez),
            Field::new("hard_storage_limit_per_operation", Encoding::Z),
            Field::new("test_chain_duration", Encoding::Int64),
            Field::new("quorum_min", Encoding::Int32),
            Field::new("quorum_max", Encoding::Int32),
            Field::new("min_proposal_quorum", Encoding::Int32),
            Field::new("initial_endorsers", Encoding::Uint16),
            Field::new("delay_per_missing_endorsement", Encoding::Int64),
        ])
});

impl ToRpcJsonMap for ParametricConstants {
    fn as_map(&self) -> HashMap<&'static str, UniversalValue> {
        let mut ret: HashMap<&'static str, UniversalValue> = Default::default();
        ret.insert("preserved_cycles", UniversalValue::num(self.preserved_cycles));
        ret.insert("blocks_per_cycle", UniversalValue::num(self.blocks_per_cycle));
        ret.insert("blocks_per_commitment", UniversalValue::num(self.blocks_per_commitment));
        ret.insert("blocks_per_roll_snapshot", UniversalValue::num(self.blocks_per_roll_snapshot));
        ret.insert("blocks_per_voting_period", UniversalValue::num(self.blocks_per_voting_period));
        ret.insert("time_between_blocks", UniversalValue::i64_list(self.time_between_blocks.clone()));
        ret.insert("endorsers_per_block", UniversalValue::num(self.endorsers_per_block));
        ret.insert("hard_gas_limit_per_operation", UniversalValue::big_num(self.hard_gas_limit_per_operation.clone()));
        ret.insert("hard_gas_limit_per_block", UniversalValue::big_num(self.hard_gas_limit_per_block.clone()));
        ret.insert("proof_of_work_threshold", UniversalValue::i64(self.proof_of_work_threshold));
        ret.insert("tokens_per_roll", UniversalValue::big_num(self.tokens_per_roll.clone()));
        ret.insert("michelson_maximum_type_size", UniversalValue::num(self.michelson_maximum_type_size));
        ret.insert("seed_nonce_revelation_tip", UniversalValue::big_num(self.seed_nonce_revelation_tip.clone()));
        ret.insert("origination_size", UniversalValue::num(self.origination_size));
        ret.insert("block_security_deposit", UniversalValue::big_num(self.block_security_deposit.clone()));
        ret.insert("endorsement_security_deposit", UniversalValue::big_num(self.endorsement_security_deposit.clone()));
        ret.insert("baking_reward_per_endorsement", UniversalValue::big_num_list(self.baking_reward_per_endorsement.clone()));
        ret.insert("endorsement_reward", UniversalValue::big_num_list(self.endorsement_reward.clone()));
        ret.insert("cost_per_byte", UniversalValue::big_num(self.cost_per_byte.clone()));
        ret.insert("hard_storage_limit_per_operation", UniversalValue::big_num(self.hard_storage_limit_per_operation.clone()));
        ret.insert("test_chain_duration", UniversalValue::i64(self.test_chain_duration));
        ret.insert("quorum_min", UniversalValue::num(self.quorum_min));
        ret.insert("quorum_max", UniversalValue::num(self.quorum_max));
        ret.insert("min_proposal_quorum", UniversalValue::num(self.min_proposal_quorum));
        ret.insert("initial_endorsers", UniversalValue::num(self.initial_endorsers));
        ret.insert("delay_per_missing_endorsement", UniversalValue::i64(self.delay_per_missing_endorsement));
        ret
    }
}