// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT


use std::convert::TryInto;
use std::string::ToString;

use failure::bail;
use itertools::Itertools;

use storage::num_from_slice;
use storage::skip_list::Bucket;
use storage::persistent::ContextMap;
use storage::context::{TezedgeContext, ContextIndex, ContextApi};
use storage::context_action_storage::contract_id_to_contract_address_for_index;
use tezos_messages::base::signature_public_key_hash::SignaturePublicKeyHash;
use tezos_messages::protocol::{RpcJsonMap, ToRpcJsonMap, UniversalValue, ToRpcJsonList};
use tezos_messages::protocol::proto_006::delegate::{BalanceByCycle, Delegate, DelegateList};
use tezos_messages::p2p::binary_message::BinaryMessage;
use num_bigint::{BigInt, ToBigInt};

use crate::helpers::ContextProtocolParam;
use crate::services::protocol::proto_006::helpers::{create_index_from_contract_id, from_zarith, cycle_from_level, DelegateActivity, DelegateRawContextData, DelegatedContracts};
use crate::services::protocol::Activity;
use crate::merge_slices;


// key pre and postfixes for context database
const KEY_POSTFIX_BALANCE: &str = "balance";
const KEY_POSTFIX_FROZEN_BALANCE: &str = "frozen_balance";
const KEY_POSTFIX_DEPOSITS: &str = "deposits";
const KEY_POSTFIX_FEES: &str = "fees";
const KEY_POSTFIX_REWARDS: &str = "rewards";
const KEY_POSTFIX_DELEGATED: &str = "delegated";
const KEY_POSTFIX_INACTIVE: &str = "inactive_delegate";
const KEY_POSTFIX_GRACE_PERIOD: &str = "delegate_desactivation";
const KEY_POSTFIX_CHANGE: &str= "change";

fn get_activity(context: &TezedgeContext, context_index: &ContextIndex, pkh: &str) -> Result<DelegateActivity, failure::Error> {
    let deactivated;
    let grace_period;

    let contract_key = construct_indexed_key(&pkh)?;
    let activity_key = vec![contract_key.clone(), KEY_POSTFIX_INACTIVE.to_string()];
    let grace_period_key = vec![contract_key, KEY_POSTFIX_GRACE_PERIOD.to_string()];


    if let Some(Bucket::Exists(_)) = context.get_key(&context_index, &activity_key)? {
        deactivated = true;
    } else {
        deactivated = false;
    }

    if let Some(Bucket::Exists(val)) = context.get_key(&context_index, &grace_period_key)? {
        grace_period = num_from_slice!(val, 0, i32);
    } else {
        bail!("Grace period for {} is not set even though it should be", pkh);
    }
    Ok(DelegateActivity::new(grace_period, deactivated))
}

pub(crate) fn list_delegates(context_proto_params: ContextProtocolParam, _chain_id: &str, activity: Activity, context: TezedgeContext) -> Result<Option<UniversalValue>, failure::Error> {
    let context_index = ContextIndex::new(Some(context_proto_params.level.clone()), None);
    let dynamic = tezos_messages::protocol::proto_006::constants::ParametricConstants::from_bytes(context_proto_params.constants_data)?;

    let blocks_per_cycle = dynamic.blocks_per_cycle();
    let block_level = context_proto_params.level;
    let block_cycle = cycle_from_level(block_level.try_into()?, blocks_per_cycle)?;

    let delegates: ContextMap;
    if let Some(val) = context.get_by_key_prefix(&context_index, &vec!["data".to_string(), "delegates/".to_string()])? {
        delegates = val;
    } else {
        bail!("Delegate keys not found");
    }

    let mut inactive_delegates: DelegateList = Default::default();
    let mut active_delegates: DelegateList = Default::default();

    for (key, _) in delegates.into_iter() {
        let address = key.split("/").skip(3).take(6).join("");
        let curve = key.split("/").skip(2).take(1).join("");

        let pkh = SignaturePublicKeyHash::from_hex_hash_and_curve(&address, &curve)?.to_string();

        let delegate_activity: DelegateActivity = get_activity(&context, &context_index, &pkh)?;

        if delegate_activity.is_active(block_cycle) {
            active_delegates.push(pkh);
        } else {
            inactive_delegates.push(pkh);
        }
    }

    match activity {
        Activity::Active => {
            active_delegates.sort();
            active_delegates.reverse();
            Ok(Some(active_delegates.as_list()))
        },
        Activity::Inactive => {
            inactive_delegates.sort();
            inactive_delegates.reverse();
            Ok(Some(inactive_delegates.as_list()))
        },
        Activity::Both => {
            let mut all_delegates = merge_slices!(&active_delegates, &inactive_delegates);
            all_delegates.sort();
            all_delegates.reverse();
            Ok(Some(all_delegates.as_list()))
        }
    }
}

fn construct_indexed_key(pkh: &str) -> Result<String, failure::Error> {
    const KEY_PREFIX: &str = "data/contracts/index";
    let index = create_index_from_contract_id(pkh)?.join("/");
    let key = hex::encode(contract_id_to_contract_address_for_index(pkh)?);

    Ok(format!("{}/{}/{}", KEY_PREFIX, index, key))
}

/// Get all the relevant data from the context
fn get_delegate_context_data(context_proto_params: ContextProtocolParam, context: &TezedgeContext, pkh: &str) -> Result<DelegateRawContextData, failure::Error> {
    let block_level = context_proto_params.level;
    let dynamic = tezos_messages::protocol::proto_006::constants::ParametricConstants::from_bytes(context_proto_params.constants_data)?;
    let preserved_cycles = dynamic.preserved_cycles();
    let blocks_per_cycle = dynamic.blocks_per_cycle();
    let context_index = ContextIndex::new(Some(context_proto_params.level.clone()), None);
    
    let block_cycle = cycle_from_level(block_level.try_into()?, blocks_per_cycle)?;
    
    let delegate_contract_key = construct_indexed_key(pkh)?;

    let balance_key = vec![delegate_contract_key.clone(), KEY_POSTFIX_BALANCE.to_string()];
    let change_key = vec![delegate_contract_key.clone(), KEY_POSTFIX_CHANGE.to_string()];

    let balance: BigInt;
    let mut frozen_balance_by_cycle: Vec<BalanceByCycle> = Vec::new();
    let change: BigInt;

    if let Some(Bucket::Exists(data)) = context.get_key(&context_index, &balance_key)? {
        balance = from_zarith(&data)?;
        
    } else {
        bail!("Balance not found");
    }
    if let Some(Bucket::Exists(data)) =  context.get_key(&context_index, &change_key)? {
        change = from_zarith(&data)?;
    } else {
        change = ToBigInt::to_bigint(&0).unwrap();
    }

    let mut activity: DelegateActivity = get_activity(&context, &context_index, pkh)?;

    // frozen balance

    for cycle in block_cycle - preserved_cycles as i64..block_cycle + 1 {
        if cycle >= 0 {
            let frozen_balance_key = format!("{}/{}/{}", delegate_contract_key, KEY_POSTFIX_FROZEN_BALANCE, cycle);

            let frozen_balance_deposits_key = vec![frozen_balance_key.clone(), KEY_POSTFIX_DEPOSITS.to_string()];
            let frozen_balance_fees_key = vec![frozen_balance_key.clone(), KEY_POSTFIX_FEES.to_string()];
            let frozen_balance_rewards_key = vec![frozen_balance_key.clone(), KEY_POSTFIX_REWARDS.to_string()];

            
            let frozen_balance_fees: BigInt;
            let frozen_balance_deposits: BigInt;
            let frozen_balance_rewards: BigInt;

            let mut found_flag: bool = false;
            // get the frozen balance data for preserved cycles and the current one
            if let Some(Bucket::Exists(data)) =  context.get_key(&context_index, &frozen_balance_deposits_key)? {
                frozen_balance_deposits = from_zarith(&data)?;
                found_flag = true;
            } else {
                frozen_balance_deposits = Default::default();
            }
            if let Some(Bucket::Exists(data)) =  context.get_key(&context_index, &frozen_balance_fees_key)? {
                frozen_balance_fees = from_zarith(&data)?;
                found_flag = true;
            } else {
                frozen_balance_fees = Default::default();
            }
            if let Some(Bucket::Exists(data)) =  context.get_key(&context_index, &frozen_balance_rewards_key)? {
                frozen_balance_rewards = from_zarith(&data)?;
                found_flag = true;
            } else {
                frozen_balance_rewards = Default::default();
            }
            // ocaml behavior
            // corner case - carthagenet - blocks <1, 6> including an empty array
            // in block 7, deposits and rewards are set, so push to vector with the fetched values and set the rest to default
            // we should push to this vec only when at least one value is found (is set in context) otherwise do not push
            if found_flag {
                frozen_balance_by_cycle.push(BalanceByCycle::new(cycle.try_into()?, frozen_balance_deposits.try_into()?, frozen_balance_fees.try_into()?, frozen_balance_rewards.try_into()?));
            }
        }
    }
    // Full key to the delegated balances looks like the following
    // "data/contracts/index/ad/af/43/23/f9/3e/000003cb7d7842406496fc07288635562bfd17e176c4/delegated/72/71/28/a2/ba/a4/000049c9bce2a9d04f7b38d32398880d96e8756a1d5c"
    // we get all delegated contracts to the delegate by filtering the context with prefix:
    // "data/contracts/index/ad/af/43/23/f9/3e/000003cb7d7842406496fc07288635562bfd17e176c4/delegated"
    let delegated_contracts_key_prefix = vec![delegate_contract_key, KEY_POSTFIX_DELEGATED.to_string()];
    
    let mut delegated_contracts: DelegatedContracts = Default::default();
    if let Some(contracts) = context.get_by_key_prefix(&context_index, &delegated_contracts_key_prefix)? {
        let mut delegated_contracts_raw = contracts.into_iter()
            .filter(|(_, v)| if let Bucket::Exists(_) = v { true } else { false })
            .map(|(k, _)| k.to_string())
            .collect::<DelegatedContracts>();
        delegated_contracts_raw.sort();
        delegated_contracts_raw.reverse();
        for contract in delegated_contracts_raw {
            if let Some(address) = contract.split("/").last() {
                delegated_contracts.push(SignaturePublicKeyHash::from_tagged_hex_string(&address)?.to_string());
            }
        }
    }

    // last activity check, we need to check for the grace_period, if the current cycle is past the delegates grace period
    // set the deactivated flag to true
    // Note: in the context, the key is not set
    if !activity.is_active(block_cycle) {
        activity.set_deactivated(true);
    }

    Ok(DelegateRawContextData::new(balance, change, frozen_balance_by_cycle, delegated_contracts, activity))
}

fn get_roll_count(level: usize, pkh: &str, context: &TezedgeContext) -> Result<usize, failure::Error> {
    let context_index = ContextIndex::new(Some(level), None);

    // simple counter to count the number of rolls the delegate owns
    let mut roll_count: usize = 0;

    let roll_key = vec![
        "data".to_string(),
        "rolls".to_string(),
        "owner".to_string(),
        "current".to_string(),
        ];
    
    if let Some(data) = context.get_by_key_prefix(&context_index, &roll_key)? {
        // iterate through all the owners,the roll_num is the last component of the key, decode the value (it is a public key) to get the public key hash address (tz1...)
        for (_, value) in data.into_iter() {
            // the values are public keys
            if let Bucket::Exists(pk) = value {
                let delegate = SignaturePublicKeyHash::from_tagged_bytes(pk)?.to_string();
                if delegate.eq(pkh) {
                    roll_count += 1;
                }
            }
        }
    } else {
        println!("No roll owners found");
    }
    Ok(roll_count)
}

pub(crate) fn get_delegate(context_proto_params: ContextProtocolParam, _chain_id: &str, pkh: &str, context: TezedgeContext) -> Result<Option<RpcJsonMap>, failure::Error> {
    // get block level first
    let block_level = context_proto_params.level;
    let dynamic = tezos_messages::protocol::proto_006::constants::ParametricConstants::from_bytes(context_proto_params.constants_data.clone())?;
    let tokens_per_roll: BigInt = dynamic.tokens_per_roll().try_into()?;

    // fetch delegate data from the context
    let delegate_data = get_delegate_context_data(context_proto_params, &context, pkh)?;

    // staking_balance
    let roll_count = get_roll_count(block_level, pkh, &context)?;
    
    let staking_balance: BigInt;
    staking_balance = tokens_per_roll * roll_count + delegate_data.change();

    // delegated balance

    // calculate the sums of deposits, fees an rewards accross all preserved cycles, including the current one
    // unwraps are safe, we are creating a BigInt from zero, so it allways fits
    let frozen_deposits: BigInt = delegate_data.frozen_balance_by_cycle().iter()
        .map(|val| val.deposit().into())
        .fold(ToBigInt::to_bigint(&0).unwrap(), |acc, elem: BigInt| acc + elem);

    let frozen_fees: BigInt = delegate_data.frozen_balance_by_cycle().iter()
        .map(|val| val.fees().into())
        .fold(ToBigInt::to_bigint(&0).unwrap(), |acc, elem: BigInt| acc + elem);

    let frozen_rewards: BigInt = delegate_data.frozen_balance_by_cycle().iter()
        .map(|val| val.rewards().into())
        .fold(ToBigInt::to_bigint(&0).unwrap(), |acc, elem: BigInt| acc + elem);

    // delegated balance is calculetd by subtracting the sum of balance frozen_deposits and frozen_fees
    // balance in this context means the spendable balance
    let delegated_balance: BigInt = &staking_balance - (delegate_data.balance() + &frozen_deposits + &frozen_fees);

    // frozen balance is the sum of all the frozen items
    let frozen_balance: BigInt = frozen_deposits + frozen_fees + frozen_rewards;

    // full balance includes frozen balance as well
    let full_balance: BigInt = &frozen_balance + delegate_data.balance();
    
    let delegates = Delegate::new(
        full_balance.try_into()?,
        frozen_balance.try_into()?,
        delegate_data.frozen_balance_by_cycle().clone(),
        staking_balance.try_into()?,
        delegate_data.delegator_list().clone(),
        delegated_balance.try_into()?,
        delegate_data.delegate_activity().deactivated(),
        delegate_data.delegate_activity().grace_period(),
    );
    
    Ok(Some(delegates.as_map()))
}