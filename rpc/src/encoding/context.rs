use serde::{Serialize, Deserialize};
use crypto::hash::ProtocolHash;
use failure::Error;
use tezos_messages::protocol::UniversalValue;
use getset::Getters;

#[derive(Serialize, Deserialize, Debug, Clone, Getters)]
#[serde(default)]
pub struct ContextConstants {
    proof_of_work_nonce_size: i64,
    #[get = "pub(crate)"]
    nonce_length: i64,
    max_revelations_per_block: i64,
    max_operation_data_length: i64,
    max_proposals_per_delegate: i64,
    #[get = "pub(crate)"]
    preserved_cycles: i64,
    #[get = "pub(crate)"]
    blocks_per_cycle: i64,
    blocks_per_commitment: i64,
    #[get = "pub(crate)"]
    blocks_per_roll_snapshot: i64,
    blocks_per_voting_period: i64,
    #[get = "pub(crate)"]
    time_between_blocks: Vec<String>,
    #[get = "pub(crate)"]
    endorsers_per_block: i64,
    hard_gas_limit_per_operation: String,
    hard_gas_limit_per_block: String,
    proof_of_work_threshold: String,
    tokens_per_roll: String,
    michelson_maximum_type_size: i64,
    seed_nonce_revelation_tip: String,
    origination_size: i64,
    block_security_deposit: String,
    endorsement_security_deposit: String,
    block_reward: String,
    endorsement_reward: String,
    cost_per_byte: String,
    hard_storage_limit_per_operation: String,
    test_chain_duration: String,
    quorum_min: i64,
    quorum_max: i64,
    min_proposal_quorum: i64,
    initial_endorsers: i64,
    delay_per_missing_endorsement: String,
}

impl Default for ContextConstants {
    fn default() -> Self {
        Self {
            proof_of_work_nonce_size: 0,
            nonce_length: 0,
            max_revelations_per_block: 0,
            max_operation_data_length: 0,
            max_proposals_per_delegate: 0,
            preserved_cycles: 0,
            blocks_per_cycle: 0,
            blocks_per_commitment: 0,
            blocks_per_roll_snapshot: 0,
            blocks_per_voting_period: 0,
            time_between_blocks: Vec::new(),
            endorsers_per_block: 0,
            hard_gas_limit_per_operation: "0".to_string(),
            hard_gas_limit_per_block: "0".to_string(),
            proof_of_work_threshold: "0".to_string(),
            tokens_per_roll: "0".to_string(),
            michelson_maximum_type_size: 0,
            seed_nonce_revelation_tip: "0".to_string(),
            origination_size: 0,
            block_security_deposit: "0".to_string(),
            endorsement_security_deposit: "0".to_string(),
            block_reward: "0".to_string(),
            endorsement_reward: "0".to_string(),
            cost_per_byte: "0".to_string(),
            hard_storage_limit_per_operation: "0".to_string(),
            test_chain_duration: "0".to_string(),
            quorum_min: 0,
            quorum_max: 0,
            min_proposal_quorum: 0,
            initial_endorsers: 0,
            delay_per_missing_endorsement: "0".to_string(),
        }
    }
}

impl ContextConstants {
    // TODO: refactor
    pub fn transpile_context_bytes(value: &[u8], protocol: ProtocolHash) -> Result<Self, Error> {
        let consts = tezos_messages::protocol::get_constants_for_rpc(value, protocol)?;
        let mut ret: Self = Default::default();
        if let Some(consts) = consts {
            // if let Some(UniversalValue::Number(num)) = consts.get("proof_of_work_nonce_size") {
            //     ret.proof_of_work_nonce_size = num.clone();
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("nonce_length") {
            //     ret.nonce_length = num.clone();
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("max_revelations_per_block") {
            //     ret.max_revelations_per_block = num.clone();
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("max_operation_data_length") {
            //     ret.max_operation_data_length = num.clone();
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("max_proposals_per_delegate") {
            //     ret.max_proposals_per_delegate = num.clone();
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("preserved_cycles") {
            //     ret.preserved_cycles = num.clone();
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("blocks_per_cycle") {
            //     ret.blocks_per_cycle = num.clone();
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("blocks_per_commitment") {
            //     ret.blocks_per_commitment = num.clone();
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("blocks_per_roll_snapshot") {
            //     ret.blocks_per_roll_snapshot = num.clone();
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("blocks_per_voting_period") {
            //     ret.blocks_per_voting_period = num.clone();
            // }
            // if let Some(UniversalValue::List(values)) = consts.get("time_between_blocks") {
            //     for x in values {
            //         if let UniversalValue::Number(num) = **x {
            //             ret.time_between_blocks.push(format!("{}", num));
            //         }
            //     }
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("endorsers_per_block") {
            //     ret.endorsers_per_block = num.clone();
            // }
            // if let Some(UniversalValue::BigNumber(num)) = consts.get("hard_gas_limit_per_operation") {
            //     ret.hard_gas_limit_per_operation = format!("{}", num.0);
            // }
            // if let Some(UniversalValue::BigNumber(num)) = consts.get("hard_gas_limit_per_block") {
            //     ret.hard_gas_limit_per_block = format!("{}", num.0);
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("proof_of_work_threshold") {
            //     ret.proof_of_work_threshold = format!("{}", num);
            // }
            // if let Some(UniversalValue::BigNumber(num)) = consts.get("tokens_per_roll") {
            //     ret.tokens_per_roll = format!("{}", num.0);
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("michelson_maximum_type_size") {
            //     ret.michelson_maximum_type_size = num.clone();
            // }
            // if let Some(UniversalValue::BigNumber(num)) = consts.get("seed_nonce_revelation_tip") {
            //     ret.seed_nonce_revelation_tip = format!("{}", num.0);
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("origination_size") {
            //     ret.origination_size = num.clone();
            // }
            // if let Some(UniversalValue::BigNumber(num)) = consts.get("block_security_deposit") {
            //     ret.block_security_deposit = format!("{}", num.0);
            // }
            // if let Some(UniversalValue::BigNumber(num)) = consts.get("endorsement_security_deposit") {
            //     ret.endorsement_security_deposit = format!("{}", num.0);
            // }
            // if let Some(UniversalValue::BigNumber(num)) = consts.get("block_reward") {
            //     ret.block_reward = format!("{}", num.0);
            // }
            // // TODO: toto minimalne nesedi
            // if let Some(UniversalValue::BigNumber(num)) = consts.get("endorsement_reward") {
            //     ret.endorsement_reward = format!("{}", num.0);
            // }
            // if let Some(UniversalValue::BigNumber(num)) = consts.get("cost_per_byte") {
            //     ret.cost_per_byte = format!("{}", num.0);
            // }
            // if let Some(UniversalValue::BigNumber(num)) = consts.get("hard_storage_limit_per_operation") {
            //     ret.hard_storage_limit_per_operation = format!("{}", num.0);
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("test_chain_duration") {
            //     ret.test_chain_duration = format!("{}", num);
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("quorum_min") {
            //     ret.quorum_min = num.clone();
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("quorum_max") {
            //     ret.quorum_max = num.clone();
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("min_proposal_quorum") {
            //     ret.min_proposal_quorum = num.clone();
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("initial_endorsers") {
            //     ret.initial_endorsers = num.clone();
            // }
            // if let Some(UniversalValue::Number(num)) = consts.get("delay_per_missing_endorsement") {
            //     ret.delay_per_missing_endorsement = format!("{}", num);
            // }
        }
        Ok(ret)
    }
}
