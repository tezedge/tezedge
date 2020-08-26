// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};

use tezos_encoding::{
    encoding::{Encoding, Field, FieldName, HasEncoding},
    has_encoding,
    types::BigInt,
};

use crate::non_cached_data;
use crate::protocol::{ToRpcJsonMap, UniversalValue};

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
        ret.insert("proof_of_work_nonce_size", UniversalValue::num(self.proof_of_work_nonce_size));
        ret.insert("nonce_length", UniversalValue::num(self.nonce_length));
        ret.insert("max_revelations_per_block", UniversalValue::num(self.max_revelations_per_block));
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
    block_reward: BigInt,
    endorsement_reward: BigInt,
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
            Field::new(FieldName::PreservedCycles, Encoding::Uint8),
            Field::new(FieldName::BlocksPerCycle, Encoding::Int32),
            Field::new(FieldName::BlocksPerCommitment, Encoding::Int32),
            Field::new(FieldName::BlocksPerRollSnapshot, Encoding::Int32),
            Field::new(FieldName::BlocksPerVotingPeriod, Encoding::Int32),
            Field::new(FieldName::TimeBetweenBlocks, Encoding::dynamic(Encoding::list(Encoding::Int64))),
            Field::new(FieldName::EndorsersPerBlock, Encoding::Uint16),
            Field::new(FieldName::HardGasLimitPerOperation, Encoding::Z),
            Field::new(FieldName::HardGasLimitPerBlock, Encoding::Z),
            Field::new(FieldName::ProofOfWorkThreshold, Encoding::Int64),
            Field::new(FieldName::TokensPerRoll, Encoding::Mutez),
            Field::new(FieldName::MichelsonMaximumTypeSize, Encoding::Uint16),
            Field::new(FieldName::SeedNonceRevelationTip, Encoding::Mutez),
            Field::new(FieldName::OriginationSize, Encoding::Int32),
            Field::new(FieldName::BlockSecurityDeposit, Encoding::Mutez),
            Field::new(FieldName::EndorsementSecurityDeposit, Encoding::Mutez),
            Field::new(FieldName::BlockReward, Encoding::Mutez),
            Field::new(FieldName::EndorsementReward, Encoding::Mutez),
            Field::new(FieldName::CostPerByte, Encoding::Mutez),
            Field::new(FieldName::HardStorageLimitPerOperation, Encoding::Z),
            Field::new(FieldName::TestChainDuration, Encoding::Int64),
            Field::new(FieldName::QuorumMin, Encoding::Int32),
            Field::new(FieldName::QuorumMax, Encoding::Int32),
            Field::new(FieldName::MinProposalQuorum, Encoding::Int32),
            Field::new(FieldName::InitialEndorsers, Encoding::Uint16),
            Field::new(FieldName::DelayPerMissingEndorsement, Encoding::Int64),
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
        ret.insert("block_reward", UniversalValue::big_num(self.block_reward.clone()));
        ret.insert("endorsement_reward", UniversalValue::big_num(self.endorsement_reward.clone()));
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