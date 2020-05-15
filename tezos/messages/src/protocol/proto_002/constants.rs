// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
use std::collections::HashMap;

use getset::{CopyGetters, Getters, Setters};
use serde::{Deserialize, Serialize};

use tezos_encoding::{
    encoding::{Encoding, Field, HasEncoding},
    types::BigInt,
};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};
use crate::protocol::{ToRpcJsonMap, UniversalValue};

pub const FIXED: FixedConstants = FixedConstants {
    proof_of_work_nonce_size: 8,
    nonce_length: 32,
    max_revelations_per_block: 32,
    max_operation_data_length: 16 * 1024,
};

// ------- Fixed Constants ------- //

#[derive(Serialize, Deserialize, Debug, Clone, Getters, CopyGetters)]
pub struct FixedConstants {
    proof_of_work_nonce_size: u8,
    #[get_copy = "pub"]
    nonce_length: u8,
    max_revelations_per_block: u8,
    max_operation_data_length: i32,
}

impl ToRpcJsonMap for FixedConstants {
    fn as_map(&self) -> HashMap<&'static str, UniversalValue> {
        let mut ret: HashMap<&'static str, UniversalValue> = Default::default();
        ret.insert("proof_of_work_nonce_size", UniversalValue::num(self.proof_of_work_nonce_size));
        ret.insert("nonce_length", UniversalValue::num(self.nonce_length));
        ret.insert("max_revelations_per_block", UniversalValue::num(self.max_revelations_per_block));
        ret.insert("max_operation_data_length", UniversalValue::num(self.max_operation_data_length));
        ret
    }
}

// ------- Parametric Constants ------- //

#[derive(Serialize, Deserialize, Debug, Clone, Getters, CopyGetters, Setters)]
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
    time_between_blocks: Option<Vec<i64>>,
    #[get_copy = "pub"]
    endorsers_per_block: Option<u16>,
    hard_gas_limit_per_operation: Option<BigInt>,
    hard_gas_limit_per_block: Option<BigInt>,
    proof_of_work_threshold: Option<i64>,
    tokens_per_roll: Option<BigInt>,
    michelson_maximum_type_size: Option<u16>,
    seed_nonce_revelation_tip: Option<BigInt>,
    origination_burn: Option<BigInt>,
    #[get = "pub"]
    #[set = "pub"]
    block_security_deposit: Option<BigInt>,
    #[get = "pub"]
    #[set = "pub"]
    endorsement_security_deposit: Option<BigInt>,
    #[get = "pub"]
    #[set = "pub"]
    block_reward: Option<BigInt>,
    #[get = "pub"]
    #[set = "pub"]
    endorsement_reward: Option<BigInt>,
    cost_per_byte: Option<BigInt>,
    hard_storage_limit_per_operation: Option<BigInt>,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl ParametricConstants {
    /// Merge the default values with the values set in context DB 
    pub fn create_with_default(context_param_constants: Self) -> Self {
        let mut param: Self = Default::default();

        if context_param_constants.block_security_deposit.is_some() {
            param.set_block_security_deposit(context_param_constants.block_security_deposit);
        }
        if context_param_constants.endorsement_security_deposit.is_some() {
            param.set_endorsement_security_deposit(context_param_constants.endorsement_security_deposit);
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
            time_between_blocks: Some(vec![60 as i64, 75 as i64]),
            endorsers_per_block: Some(32),
            hard_gas_limit_per_operation: Some(num_bigint::BigInt::from(400_000).into()),
            hard_gas_limit_per_block: Some(num_bigint::BigInt::from(4_000_000).into()),
            proof_of_work_threshold: Some(70_368_744_177_663),
            tokens_per_roll: Some(num_bigint::BigInt::from(10_000_000_000 as i64).into()),
            michelson_maximum_type_size: Some(1000),
            seed_nonce_revelation_tip: Some(num_bigint::BigInt::from(125000).into()),
            origination_burn: Some(num_bigint::BigInt::from(257000).into()),
            
            // can me modified
            block_security_deposit: Some(num_bigint::BigInt::from(0).into()),
            
            // can me modified
            endorsement_security_deposit: Some(num_bigint::BigInt::from(0).into()),
            
            // can me modified
            block_reward: Some(num_bigint::BigInt::from(0).into()),

            // can me modified
            endorsement_reward: Some(num_bigint::BigInt::from(0).into()),

            cost_per_byte: Some(num_bigint::BigInt::from(1000).into()),
            hard_storage_limit_per_operation: Some(num_bigint::BigInt::from(60000).into()),

            body: Default::default()
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
            ret.insert("blocks_per_commitment", UniversalValue::num(blocks_per_commitment));
        }
        if let Some(blocks_per_roll_snapshot) = self.blocks_per_roll_snapshot {
            ret.insert("blocks_per_roll_snapshot", UniversalValue::num(blocks_per_roll_snapshot));
        }
        if let Some(blocks_per_voting_period) = self.blocks_per_voting_period {
            ret.insert("blocks_per_voting_period", UniversalValue::num(blocks_per_voting_period));
        }
        if let Some(time_between_blocks) = &self.time_between_blocks {
            ret.insert("time_between_blocks", UniversalValue::i64_list(time_between_blocks.clone()));  
        }
        if let Some(endorsers_per_block) = self.endorsers_per_block {
            ret.insert("endorsers_per_block", UniversalValue::num(endorsers_per_block));
        }
        if let Some(hard_gas_limit_per_operation) = &self.hard_gas_limit_per_operation {
            ret.insert("hard_gas_limit_per_operation", UniversalValue::big_num(hard_gas_limit_per_operation.clone()));   
        }
        if let Some(hard_gas_limit_per_block) = &self.hard_gas_limit_per_block {
            ret.insert("hard_gas_limit_per_block", UniversalValue::big_num(hard_gas_limit_per_block.clone()));
        }
        if let Some(proof_of_work_threshold) = self.proof_of_work_threshold {
            ret.insert("proof_of_work_threshold", UniversalValue::i64(proof_of_work_threshold)); 
        }
        if let Some(tokens_per_roll) = &self.tokens_per_roll {
            ret.insert("tokens_per_roll", UniversalValue::big_num(tokens_per_roll.clone()));   
        }
        if let Some(michelson_maximum_type_size) = self.michelson_maximum_type_size {
            ret.insert("michelson_maximum_type_size", UniversalValue::num(michelson_maximum_type_size));
        }
        if let Some(seed_nonce_revelation_tip) = &self.seed_nonce_revelation_tip {
            ret.insert("seed_nonce_revelation_tip", UniversalValue::big_num(seed_nonce_revelation_tip.clone()));  
        }
        if let Some(origination_burn) = &self.origination_burn {
            ret.insert("origination_burn", UniversalValue::big_num(origination_burn.clone())); 
        }
        if let Some(block_security_deposit) = &self.block_security_deposit {
            ret.insert("block_security_deposit", UniversalValue::big_num(block_security_deposit.clone()));   
        }
        if let Some(endorsement_security_deposit) = &self.endorsement_security_deposit {
            ret.insert("endorsement_security_deposit", UniversalValue::big_num(endorsement_security_deposit.clone()));
        }
        if let Some(block_reward) = &self.block_reward {
            ret.insert("block_reward", UniversalValue::big_num(block_reward.clone()));
        }
        if let Some(endorsement_reward) = &self.endorsement_reward {
            ret.insert("endorsement_reward", UniversalValue::big_num(endorsement_reward.clone()));
        }
        if let Some(cost_per_byte) = &self.cost_per_byte {
            ret.insert("cost_per_byte", UniversalValue::big_num(cost_per_byte.clone()));
        }
        if let Some(hard_storage_limit_per_operation) = &self.hard_storage_limit_per_operation {
            ret.insert("hard_storage_limit_per_operation", UniversalValue::big_num(hard_storage_limit_per_operation.clone()));
        }
        // if let Some() = self. {
            
        // }
        ret
    }
}

impl HasEncoding for ParametricConstants {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("preserved_cycles", Encoding::option(Encoding::Uint8)),
            Field::new("blocks_per_cycle", Encoding::option(Encoding::Int32)),
            Field::new("blocks_per_commitment", Encoding::option(Encoding::Int32)),
            Field::new("blocks_per_roll_snapshot", Encoding::option(Encoding::Int32)),
            Field::new("blocks_per_voting_period", Encoding::option(Encoding::Int32)),
            Field::new("time_between_blocks", Encoding::option(Encoding::dynamic(Encoding::list(Encoding::Int64)))),
            Field::new("endorsers_per_block", Encoding::option(Encoding::Uint16)),
            Field::new("hard_gas_limit_per_operation", Encoding::option(Encoding::Z)),
            Field::new("hard_gas_limit_per_block", Encoding::option(Encoding::Z)),
            Field::new("proof_of_work_threshold", Encoding::option(Encoding::Int64)),
            Field::new("tokens_per_roll", Encoding::option(Encoding::Mutez)),
            Field::new("michelson_maximum_type_size", Encoding::option(Encoding::Uint16)),
            Field::new("seed_nonce_revelation_tip", Encoding::option(Encoding::Mutez)),
            Field::new("origination_burn", Encoding::option(Encoding::Mutez)),
            Field::new("block_security_deposit", Encoding::option(Encoding::Mutez)),
            Field::new("endorsement_security_deposit", Encoding::option(Encoding::Mutez)),
            Field::new("block_reward", Encoding::option(Encoding::Mutez)),
            Field::new("endorsement_reward", Encoding::option(Encoding::Mutez)),
            Field::new("cost_per_byte", Encoding::option(Encoding::Mutez)),
            Field::new("hard_storage_limit_per_operation", Encoding::option(Encoding::Z)),
        ])
    }
}

impl CachedData for ParametricConstants {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}
