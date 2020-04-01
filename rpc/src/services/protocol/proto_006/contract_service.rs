// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::string::ToString;

use failure::bail;

use storage::skip_list::Bucket;
use storage::context::{TezedgeContext, ContextIndex, ContextApi};
use tezos_messages::protocol::{RpcJsonMap, ToRpcResultJsonMap};
use tezos_messages::protocol::proto_006::contract::{Script, Contract};
// use tezos_messages::protocol::proto_006::delegate::{BalanceByCycle, Delegate, DelegateList};
use tezos_messages::base::signature_public_key_hash::SignaturePublicKeyHash;
use tezos_messages::p2p::binary_message::BinaryMessage;
//use tezos_messages::protocol::proto_006::

use crate::helpers::ContextProtocolParam;
use crate::services::protocol::proto_006::helpers::{construct_indexed_contract_key};

pub(crate) fn get_contract(context_proto_params: ContextProtocolParam, _chain_id: &str, pkh: &str, context: TezedgeContext) -> Result<Option<RpcJsonMap>, failure::Error> {

    // level of the block
    let level = context_proto_params.level;

    let context_index = ContextIndex::new(Some(level), None);
    
    let indexed_contract_key = construct_indexed_contract_key(pkh)?;
    
    let balance_key = vec![indexed_contract_key.clone(), "balance".to_string()];
    let balance;
    // ["data","contracts","index","91","6e","d7","72","4e","49","0000535110affdb82923710d1ec205f26ba8820a2259","balance"]
    if let Some(Bucket::Exists(data)) = context.get_key(&context_index, &balance_key)? {
        balance = tezos_messages::protocol::proto_006::contract::Balance::from_bytes(data)?;
    } else {
        bail!("Balance not found");
    }

    // ["data","contracts","index","91","6e","d7","72","4e","49","0000535110affdb82923710d1ec205f26ba8820a2259","data","code"]
    let contract_script_code;
    let contract_script_code_key = vec![indexed_contract_key.clone(), "data/code".to_string()];
    if let Some(Bucket::Exists(data)) = context.get_key(&context_index, &contract_script_code_key)? {
        contract_script_code = Some(tezos_messages::protocol::proto_006::contract::Code::from_bytes(data)?);
    } else {
        // Set the value to default as implicit contracts have no script attached
        contract_script_code = None;
    }

    // ["data","contracts","index","91","6e","d7","72","4e","49","0000535110affdb82923710d1ec205f26ba8820a2259","data","storage"]
    let contract_script_storage;
    let contract_script_storage_key = vec![indexed_contract_key.clone(), "data/storage".to_string()];
    if let Some(Bucket::Exists(data)) = context.get_key(&context_index, &contract_script_storage_key)? {
        contract_script_storage = Some(tezos_messages::protocol::proto_006::contract::Storage::from_bytes(data)?);
    } else {
        // Set the value to default as implicit contracts have no script attached
        contract_script_storage = None;
    }

    // ["data","contracts","index","91","6e","d7","72","4e","49","0000535110affdb82923710d1ec205f26ba8820a2259","counter"]
    let contract_counter;
    let contract_counter_key = vec![indexed_contract_key.clone(), "counter".to_string()];
    if let Some(Bucket::Exists(data)) = context.get_key(&context_index, &contract_counter_key)? {
        contract_counter = Some(tezos_messages::protocol::proto_006::contract::Counter::from_bytes(data)?);
    } else {
        // Set the value to default as implicit contracts have no script attached
        contract_counter = None;
    }

    let contract_delegate;
    let contract_delegate_key = vec![indexed_contract_key.clone(), "delegate".to_string()];
    if let Some(Bucket::Exists(data)) = context.get_key(&context_index, &contract_delegate_key)? {
        contract_delegate = Some(SignaturePublicKeyHash::from_tagged_hex_string(&hex::encode(&data))?);
    } else {
        // Set the value to default as implicit contracts have no script attached
        contract_delegate = None;
    }

    // match (contract_script_code, contract_script_storage) {
    //     (Some(contract_script_code), Some(contract_script_storage)) => Script::new(contract_script_code, contract_script_storage),
    //     _ => 
    // };
    // // let script = Script::new(contract_script_code, contract_script_storage)

    // // let mut value_vec: Vec<Box<UniversalValue>> = Default::default();
    
    // construct Contract object
    let script = if let (Some(contract_script_code), Some(contract_script_storage)) = (contract_script_code, contract_script_storage){
        Some(Script::new(contract_script_code, contract_script_storage))
    } else {
        None
    };

    let contract = Contract::new(balance.balance().clone(), contract_delegate, script, contract_counter);

    Ok(Some(contract.as_result_map()?))
}