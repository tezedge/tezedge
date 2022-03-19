// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use getset::{CopyGetters, Getters, Setters};
use serde::{Deserialize, Serialize};

use tezos_encoding::nom::NomReader;
use tezos_encoding::{
    encoding::HasEncoding,
    types::{Mutez, Zarith},
};

use crate::base::rpc_support::{ToRpcJsonMap, UniversalValue};

pub const FIXED: FixedConstants = FixedConstants {
    proof_of_work_nonce_size: 8,
    nonce_length: 32,
    max_revelations_per_block: 32,
    max_operation_data_length: 16 * 1024,
    max_proposals_per_delegate: 20,
};

#[derive(Serialize, Deserialize, Debug, Clone, Getters, CopyGetters)]
pub struct FixedConstants {
    proof_of_work_nonce_size: u8,
    #[get_copy = "pub"]
    nonce_length: u8,
    max_revelations_per_block: u8,
    max_operation_data_length: i32,
    max_proposals_per_delegate: u8,
}

impl ToRpcJsonMap for FixedConstants {
    fn as_map(&self) -> HashMap<&'static str, UniversalValue> {
        let mut ret: HashMap<&'static str, UniversalValue> = Default::default();
        ret.insert(
            "proof_of_work_nonce_size",
            UniversalValue::num(self.proof_of_work_nonce_size),
        );
        ret.insert("nonce_length", UniversalValue::num(self.nonce_length));
        ret.insert(
            "max_revelations_per_block",
            UniversalValue::num(self.max_revelations_per_block),
        );
        ret.insert(
            "max_operation_data_length",
            UniversalValue::num(self.max_operation_data_length),
        );
        ret.insert(
            "max_proposals_per_delegate",
            UniversalValue::num(self.max_proposals_per_delegate),
        );
        ret
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(
    Serialize, Deserialize, Debug, Clone, Getters, CopyGetters, Setters, HasEncoding, NomReader,
)]
pub struct ParametricConstants {
    #[get_copy = "pub"]
    preserved_cycles: Option<u8>,
    #[get_copy = "pub"]
    blocks_per_cycle: Option<i32>,
    blocks_per_commitment: Option<i32>,
    #[get_copy = "pub"]
    blocks_per_roll_snapshot: Option<i32>,
    blocks_per_voting_period: Option<i32>,
    #[get = "pub"]
    #[encoding(option, dynamic, list)]
    time_between_blocks: Option<Vec<i64>>,
    #[get_copy = "pub"]
    endorsers_per_block: Option<u16>,
    hard_gas_limit_per_operation: Option<Zarith>,
    hard_gas_limit_per_block: Option<Zarith>,
    proof_of_work_threshold: Option<i64>,
    tokens_per_roll: Option<Mutez>,
    michelson_maximum_type_size: Option<u16>,
    seed_nonce_revelation_tip: Option<Mutez>,
    origination_size: Option<i32>,
    #[get = "pub"]
    #[set = "pub"]
    block_security_deposit: Option<Mutez>,
    #[get = "pub"]
    #[set = "pub"]
    endorsement_security_deposit: Option<Mutez>,
    #[get = "pub"]
    #[set = "pub"]
    block_reward: Option<Mutez>,
    #[get = "pub"]
    #[set = "pub"]
    endorsement_reward: Option<Mutez>,
    cost_per_byte: Option<Mutez>,
    hard_storage_limit_per_operation: Option<Zarith>,
}

impl ParametricConstants {
    /// Merge the default values with the values set in context DB
    pub fn create_with_default_and_merge(context_param_constants: Self) -> Self {
        let mut param: Self = Default::default();

        if context_param_constants.block_security_deposit.is_some() {
            param.set_block_security_deposit(context_param_constants.block_security_deposit);
        }
        if context_param_constants
            .endorsement_security_deposit
            .is_some()
        {
            param.set_endorsement_security_deposit(
                context_param_constants.endorsement_security_deposit,
            );
        }
        if context_param_constants.block_reward.is_some() {
            param.set_block_reward(context_param_constants.block_reward);
        }
        if context_param_constants.endorsement_reward.is_some() {
            param.set_endorsement_reward(context_param_constants.endorsement_reward);
        }

        param
    }
}

impl Default for ParametricConstants {
    fn default() -> Self {
        Self {
            preserved_cycles: Some(5),
            blocks_per_cycle: Some(4096),
            blocks_per_commitment: Some(32),
            blocks_per_roll_snapshot: Some(256),
            blocks_per_voting_period: Some(32768),
            time_between_blocks: Some(vec![60_i64, 75_i64]),
            endorsers_per_block: Some(32),
            hard_gas_limit_per_operation: Some(num_bigint::BigInt::from(400_000).into()),
            hard_gas_limit_per_block: Some(num_bigint::BigInt::from(4_000_000).into()),
            proof_of_work_threshold: Some(70_368_744_177_663),
            tokens_per_roll: Some(num_bigint::BigInt::from(10_000_000_000_i64).into()),
            michelson_maximum_type_size: Some(1000),
            seed_nonce_revelation_tip: Some(num_bigint::BigInt::from(125000).into()),
            origination_size: Some(257),

            // can me modified
            block_security_deposit: Some(num_bigint::BigInt::from(512000000).into()),

            // can me modified
            endorsement_security_deposit: Some(num_bigint::BigInt::from(64000000).into()),

            // can me modified
            block_reward: Some(num_bigint::BigInt::from(16000000).into()),

            // can me modified
            endorsement_reward: Some(num_bigint::BigInt::from(2000000).into()),

            cost_per_byte: Some(num_bigint::BigInt::from(1000).into()),
            hard_storage_limit_per_operation: Some(num_bigint::BigInt::from(60000).into()),
        }
    }
}

impl ToRpcJsonMap for ParametricConstants {
    fn as_map(&self) -> HashMap<&'static str, UniversalValue> {
        let mut ret: HashMap<&'static str, UniversalValue> = Default::default();
        if let Some(preserved_cycles) = self.preserved_cycles {
            ret.insert("preserved_cycles", UniversalValue::num(preserved_cycles));
        }
        if let Some(blocks_per_cycle) = self.blocks_per_cycle {
            ret.insert("blocks_per_cycle", UniversalValue::num(blocks_per_cycle));
        }
        if let Some(blocks_per_commitment) = self.blocks_per_commitment {
            ret.insert(
                "blocks_per_commitment",
                UniversalValue::num(blocks_per_commitment),
            );
        }
        if let Some(blocks_per_roll_snapshot) = self.blocks_per_roll_snapshot {
            ret.insert(
                "blocks_per_roll_snapshot",
                UniversalValue::num(blocks_per_roll_snapshot),
            );
        }
        if let Some(blocks_per_voting_period) = self.blocks_per_voting_period {
            ret.insert(
                "blocks_per_voting_period",
                UniversalValue::num(blocks_per_voting_period),
            );
        }
        if let Some(time_between_blocks) = &self.time_between_blocks {
            ret.insert(
                "time_between_blocks",
                UniversalValue::i64_list(time_between_blocks.clone()),
            );
        }
        if let Some(endorsers_per_block) = self.endorsers_per_block {
            ret.insert(
                "endorsers_per_block",
                UniversalValue::num(endorsers_per_block),
            );
        }
        if let Some(hard_gas_limit_per_operation) = &self.hard_gas_limit_per_operation {
            ret.insert(
                "hard_gas_limit_per_operation",
                UniversalValue::big_num(hard_gas_limit_per_operation),
            );
        }
        if let Some(hard_gas_limit_per_block) = &self.hard_gas_limit_per_block {
            ret.insert(
                "hard_gas_limit_per_block",
                UniversalValue::big_num(hard_gas_limit_per_block),
            );
        }
        if let Some(proof_of_work_threshold) = self.proof_of_work_threshold {
            ret.insert(
                "proof_of_work_threshold",
                UniversalValue::i64(proof_of_work_threshold),
            );
        }
        if let Some(tokens_per_roll) = &self.tokens_per_roll {
            ret.insert("tokens_per_roll", UniversalValue::big_num(tokens_per_roll));
        }
        if let Some(michelson_maximum_type_size) = self.michelson_maximum_type_size {
            ret.insert(
                "michelson_maximum_type_size",
                UniversalValue::num(michelson_maximum_type_size),
            );
        }
        if let Some(seed_nonce_revelation_tip) = &self.seed_nonce_revelation_tip {
            ret.insert(
                "seed_nonce_revelation_tip",
                UniversalValue::big_num(seed_nonce_revelation_tip),
            );
        }
        if let Some(origination_size) = self.origination_size {
            ret.insert("origination_size", UniversalValue::num(origination_size));
        }
        if let Some(block_security_deposit) = &self.block_security_deposit {
            ret.insert(
                "block_security_deposit",
                UniversalValue::big_num(block_security_deposit),
            );
        }
        if let Some(endorsement_security_deposit) = &self.endorsement_security_deposit {
            ret.insert(
                "endorsement_security_deposit",
                UniversalValue::big_num(endorsement_security_deposit),
            );
        }
        if let Some(block_reward) = &self.block_reward {
            ret.insert("block_reward", UniversalValue::big_num(block_reward));
        }
        if let Some(endorsement_reward) = &self.endorsement_reward {
            ret.insert(
                "endorsement_reward",
                UniversalValue::big_num(endorsement_reward),
            );
        }
        if let Some(cost_per_byte) = &self.cost_per_byte {
            ret.insert("cost_per_byte", UniversalValue::big_num(cost_per_byte));
        }
        if let Some(hard_storage_limit_per_operation) = &self.hard_storage_limit_per_operation {
            ret.insert(
                "hard_storage_limit_per_operation",
                UniversalValue::big_num(hard_storage_limit_per_operation),
            );
        }
        // if let Some() = self. {

        // }
        ret
    }
}
