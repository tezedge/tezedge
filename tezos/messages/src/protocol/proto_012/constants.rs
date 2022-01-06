// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};

use tezos_encoding::nom::NomReader;
use tezos_encoding::{
    encoding::HasEncoding,
    types::{Mutez, Zarith},
};

use crate::base::rpc_support::{ToRpcJsonMap, UniversalValue};
//use crate::base::signature_public_key::SignaturePublicKeyHash;

const STAKE_DISTRIBUTION_SIZE: i64 = 500 /* delegates */ * 15 /* words */ * 4 /* bytes */;
const SAMPLER_STATE_SIZE: i64 = 80 /* words */ * 4 /* bytes */;

pub const FIXED: FixedConstants = FixedConstants {
    proof_of_work_nonce_size: 8,
    nonce_length: 32,
    max_anon_ops_per_block: 132,
    max_operation_data_length: 16 * 1024,
    max_proposals_per_delegate: 20,
    max_micheline_node_count: 50_000,
    max_micheline_bytes_limit: 50_000,
    max_allowed_global_constants_depth: 10_000,
    cache_layout: [
        100_000_000,
        8 /* cycles */ * STAKE_DISTRIBUTION_SIZE,
        8 /* cycles */ * SAMPLER_STATE_SIZE,
    ],
    michelson_maximum_type_size: 2001,
};

#[derive(Serialize, Deserialize, Debug, Clone, CopyGetters)]
pub struct FixedConstants {
    proof_of_work_nonce_size: u8,
    #[get_copy = "pub"]
    nonce_length: u8,
    max_anon_ops_per_block: u8,
    max_operation_data_length: i32,
    max_proposals_per_delegate: u8,
    max_micheline_node_count: i32,
    max_micheline_bytes_limit: i32,
    max_allowed_global_constants_depth: i32,
    cache_layout: [i64; 3],
    michelson_maximum_type_size: u16,
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
            "max_anon_ops_per_block",
            UniversalValue::num(self.max_anon_ops_per_block),
        );
        ret.insert(
            "max_operation_data_length",
            UniversalValue::num(self.max_operation_data_length),
        );
        ret.insert(
            "max_proposals_per_delegate",
            UniversalValue::num(self.max_proposals_per_delegate),
        );
        ret.insert(
            "max_micheline_node_count",
            UniversalValue::num(self.max_micheline_node_count),
        );
        ret.insert(
            "max_micheline_bytes_limit",
            UniversalValue::num(self.max_micheline_bytes_limit),
        );
        ret.insert(
            "max_allowed_global_constants_depth",
            UniversalValue::num(self.max_allowed_global_constants_depth),
        );
        ret.insert(
            "cache_layout",
            UniversalValue::i64_list(self.cache_layout.to_vec()),
        );
        ret.insert(
            "michelson_maximum_type_size",
            UniversalValue::num(self.michelson_maximum_type_size),
        );
        ret
    }
}

// TODO
// #[derive(Serialize, Deserialize, Debug, Clone)]
// pub enum DelegateSelection {
//     Random,
//     RoundRobinOver(Vec<Vec<SignaturePublicKeyHash>>),
// }
//
// impl HasEncoding for DelegateSelection {
//     fn encoding() -> &'static tezos_encoding::encoding::Encoding {
//         todo!()
//     }
// }

#[derive(Serialize, Deserialize, Debug, Clone, HasEncoding, NomReader)]
pub struct Ratio {
    pub numerator: u16,
    pub denominator: u16,
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone, Getters, CopyGetters, HasEncoding, NomReader)]
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
    #[encoding(dynamic, list)]
    time_between_blocks: Vec<i64>,
    #[get_copy = "pub"]
    endorsers_per_block: u16,
    hard_gas_limit_per_operation: Zarith,
    hard_gas_limit_per_block: Zarith,
    proof_of_work_threshold: i64,
    tokens_per_roll: Mutez,
    michelson_maximum_type_size: u16,
    seed_nonce_revelation_tip: Mutez,
    origination_size: i32,
    block_security_deposit: Mutez,
    endorsement_security_deposit: Mutez,
    #[encoding(dynamic, list)]
    baking_reward_per_endorsement: Vec<Mutez>,
    #[encoding(dynamic, list)]
    endorsement_reward: Vec<Mutez>,
    cost_per_byte: Mutez,
    hard_storage_limit_per_operation: Zarith,
    quorum_min: i32,
    quorum_max: i32,
    min_proposal_quorum: i32,
    initial_endorsers: u16,
    delay_per_missing_endorsement: i64,
    liquidity_baking_subsidy: Mutez,
    liquidity_baking_sunset_level: i32,
    liquidity_baking_escape_ema_threshold: i32,
    max_operations_time_to_live: i16,
    minimal_block_delay: i64,
    delay_increment_per_round: i64,
    minimal_participation_ratio: Ratio,
    consensus_committee_size: i32,
    consensus_threshold: i32,
    max_slashing_period: i32,
    frozen_deposits_percentage: i32,
    double_baking_punishment: Mutez,
    ratio_of_frozen_deposits_slashed_per_double_endorsement: Ratio,
    // TODO
    // delegate_selection: DelegateSelection,
}

impl ToRpcJsonMap for ParametricConstants {
    fn as_map(&self) -> HashMap<&'static str, UniversalValue> {
        let mut ret: HashMap<&'static str, UniversalValue> = Default::default();
        ret.insert(
            "preserved_cycles",
            UniversalValue::num(self.preserved_cycles),
        );
        ret.insert(
            "blocks_per_cycle",
            UniversalValue::num(self.blocks_per_cycle),
        );
        ret.insert(
            "blocks_per_commitment",
            UniversalValue::num(self.blocks_per_commitment),
        );
        ret.insert(
            "blocks_per_roll_snapshot",
            UniversalValue::num(self.blocks_per_roll_snapshot),
        );
        ret.insert(
            "blocks_per_voting_period",
            UniversalValue::num(self.blocks_per_voting_period),
        );
        ret.insert(
            "time_between_blocks",
            UniversalValue::i64_list(self.time_between_blocks.clone()),
        );
        ret.insert(
            "endorsers_per_block",
            UniversalValue::num(self.endorsers_per_block),
        );
        ret.insert(
            "hard_gas_limit_per_operation",
            UniversalValue::big_num(self.hard_gas_limit_per_operation.clone()),
        );
        ret.insert(
            "hard_gas_limit_per_block",
            UniversalValue::big_num(self.hard_gas_limit_per_block.clone()),
        );
        ret.insert(
            "proof_of_work_threshold",
            UniversalValue::i64(self.proof_of_work_threshold),
        );
        ret.insert(
            "tokens_per_roll",
            UniversalValue::big_num(self.tokens_per_roll.clone()),
        );
        ret.insert(
            "michelson_maximum_type_size",
            UniversalValue::num(self.michelson_maximum_type_size),
        );
        ret.insert(
            "seed_nonce_revelation_tip",
            UniversalValue::big_num(self.seed_nonce_revelation_tip.clone()),
        );
        ret.insert(
            "origination_size",
            UniversalValue::num(self.origination_size),
        );
        ret.insert(
            "block_security_deposit",
            UniversalValue::big_num(self.block_security_deposit.clone()),
        );
        ret.insert(
            "endorsement_security_deposit",
            UniversalValue::big_num(self.endorsement_security_deposit.clone()),
        );
        ret.insert(
            "baking_reward_per_endorsement",
            UniversalValue::big_num_list(self.baking_reward_per_endorsement.clone()),
        );
        ret.insert(
            "endorsement_reward",
            UniversalValue::big_num_list(self.endorsement_reward.clone()),
        );
        ret.insert(
            "cost_per_byte",
            UniversalValue::big_num(self.cost_per_byte.clone()),
        );
        ret.insert(
            "hard_storage_limit_per_operation",
            UniversalValue::big_num(self.hard_storage_limit_per_operation.clone()),
        );
        ret.insert("quorum_min", UniversalValue::num(self.quorum_min));
        ret.insert("quorum_max", UniversalValue::num(self.quorum_max));
        ret.insert(
            "min_proposal_quorum",
            UniversalValue::num(self.min_proposal_quorum),
        );
        ret.insert(
            "initial_endorsers",
            UniversalValue::num(self.initial_endorsers),
        );
        ret.insert(
            "delay_per_missing_endorsement",
            UniversalValue::i64(self.delay_per_missing_endorsement),
        );
        ret.insert(
            "liquidity_baking_subsidy",
            UniversalValue::big_num(self.liquidity_baking_subsidy.clone()),
        );
        ret.insert(
            "liquidity_baking_sunset_level",
            UniversalValue::num(self.liquidity_baking_sunset_level),
        );
        ret.insert(
            "liquidity_baking_escape_ema_threshold",
            UniversalValue::num(self.liquidity_baking_escape_ema_threshold),
        );
        ret
    }
}
