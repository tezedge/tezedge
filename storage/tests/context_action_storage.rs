// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Error;

use crypto::hash::HashType;
use storage::tests_common::TmpStorage;
use storage::*;
use tezos_context::channel::ContextAction;

#[test]
fn context_get_values_by_block_hash() -> Result<(), Error> {
    let tmp_storage = TmpStorage::create("__ctx_storage_get_by_block_hash")?;

    let str_block_hash_1 = "BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET";
    let block_hash_1 = HashType::BlockHash.b58check_to_hash(str_block_hash_1)?;
    let str_block_hash_2 = "BLaf78njreWdt2WigJjM9e3ecEdVKm5ehahUfYBKvcWvZ8vfTcJ";
    let block_hash_2 = HashType::BlockHash.b58check_to_hash(str_block_hash_2)?;
    let value_1_0 = ContextAction::Set {
        key: vec![
            "hello".to_string(),
            "this".to_string(),
            "is".to_string(),
            "dog".to_string(),
        ],
        value: vec![10, 200],
        operation_hash: None,
        block_hash: Some(str_block_hash_1.into()),
        context_hash: None,
        value_as_json: None,
        start_time: 0.0,
        end_time: 0.0,
        ignored: false,
    };
    let value_1_1 = ContextAction::Set {
        key: vec!["hello".to_string(), "world".to_string()],
        value: vec![11, 200],
        operation_hash: None,
        block_hash: Some(str_block_hash_1.into()),
        context_hash: None,
        value_as_json: None,
        start_time: 0.0,
        end_time: 0.0,
        ignored: false,
    };
    let value_2_0 = ContextAction::Set {
        key: vec!["nice".to_string(), "to meet you".to_string()],
        value: vec![20, 200],
        operation_hash: None,
        block_hash: Some(str_block_hash_2.into()),
        context_hash: None,
        value_as_json: None,
        start_time: 0.0,
        end_time: 0.0,
        ignored: false,
    };
    let value_2_1 = ContextAction::Get {
        key: vec!["nice".to_string(), "to meet you".to_string()],
        value: vec![20, 200],
        operation_hash: None,
        block_hash: Some(str_block_hash_2.into()),
        context_hash: None,
        value_as_json: None,
        start_time: 0.0,
        end_time: 0.0,
    };

    let mut storage = ContextActionStorage::new(tmp_storage.storage());
    storage.put_action(&block_hash_1, value_1_0)?;
    storage.put_action(&block_hash_2, value_2_0)?;
    storage.put_action(&block_hash_1, value_1_1)?;
    storage.put_action(&block_hash_2, value_2_1)?;
    tmp_storage
        .storage()
        .kv(persistent::StorageType::Context)
        .flush()
        .unwrap();

    // block hash 1
    let values = storage.get_by_block_hash(&block_hash_1)?;
    assert_eq!(
        2,
        values.len(),
        "Was expecting vector of {} elements but instead found {}",
        2,
        values.len()
    );
    if let ContextAction::Set { value, .. } = values[0].action() {
        assert_eq!(&vec![10, 200], value);
    } else {
        panic!("Was expecting ContextAction::Set");
    }
    if let ContextAction::Set { value, .. } = values[1].action() {
        assert_eq!(&vec![11, 200], value);
    } else {
        panic!("Was expecting ContextAction::Set");
    }
    // block hash 2
    let values = storage.get_by_block_hash(&block_hash_2)?;
    assert_eq!(
        2,
        values.len(),
        "Was expecting vector of {} elements but instead found {}",
        2,
        values.len()
    );
    if let ContextAction::Set { value, .. } = values[0].action() {
        assert_eq!(&vec![20, 200], value);
    } else {
        panic!("Was expecting ContextAction::Set");
    }
    if let ContextAction::Get { key, .. } = values[1].action() {
        assert_eq!(&vec!("nice".to_string(), "to meet you".to_string()), key);
    } else {
        panic!("Was expecting ContextAction::Get");
    }

    Ok(())
}

#[test]
fn context_get_values_by_contract_address() -> Result<(), Error> {
    let tmp_storage = TmpStorage::create("__ctx_storage_get_by_contract_address")?;

    let str_block_hash = "BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET";
    let block_hash = HashType::BlockHash.b58check_to_hash(str_block_hash)?;
    let value = ContextAction::Set {
        key: vec![
            "data".to_string(),
            "contracts".to_string(),
            "index".to_string(),
            "ad".to_string(),
            "af".to_string(),
            "43".to_string(),
            "23".to_string(),
            "f9".to_string(),
            "3e".to_string(),
            "000003cb7d7842406496fc07288635562bfd17e176c4".to_string(),
            "delegate_desactivation".to_string(),
        ],
        value: vec![10, 200],
        operation_hash: None,
        block_hash: Some(str_block_hash.into()),
        context_hash: None,
        value_as_json: None,
        start_time: 0.0,
        end_time: 0.0,
        ignored: false,
    };

    let mut storage = ContextActionStorage::new(tmp_storage.storage());
    storage.put_action(&block_hash, value)?;

    // block hash 1
    let values = storage.get_by_contract_address(
        &hex::decode("000003cb7d7842406496fc07288635562bfd17e176c4")?,
        None,
        10,
    )?;
    assert_eq!(
        1,
        values.len(),
        "Was expecting vector of {} elements but instead found {}",
        1,
        values.len()
    );
    assert_eq!(0, values[0].id());
    if let ContextAction::Set { value, .. } = values[0].action() {
        assert_eq!(&vec![10, 200], value);
    } else {
        panic!("Was expecting ContextAction::Set");
    }

    Ok(())
}
