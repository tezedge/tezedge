// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module provides all the FFI callback functions.

use ocaml::{caml, List, Str, Tuple, Value};
use ocaml::value::TRUE;

use tezos_context::channel::*;

type OcamlBytes = Str;
type Hash = Vec<u8>;
type ContextHash = Hash;
type BlockHash = Hash;
type OperationHash = Hash;
type ContextKey = Vec<String>;
type ContextValue = Vec<u8>;

pub trait Interchange<T> {
    fn convert_to(&self) -> T;
}

impl Interchange<Vec<u8>> for OcamlBytes {
    fn convert_to(&self) -> ContextHash {
        self.data().to_vec()
    }
}

impl Interchange<ContextKey> for List {
    fn convert_to(&self) -> ContextKey {
        self.to_vec()
            .into_iter()
            .map(|k| Str::from(k).as_str().to_string())
            .collect()
    }
}

// Str means hash as hex string
fn to_hash(hash: Str) -> Option<Hash> {
    if hash.len() <= 0 {
        None
    } else {
        Some(hash.data().to_vec())
    }
}

fn to_string(value: Str) -> Option<String> {

    if value.len() <= 0 {
        None
    } else {
        Some(value.as_str().to_string())
    }
}

// External callback function for set value to context
caml!(ml_context_set(context_hash, block_hash, operation_hash, keyval_and_json, time_period) {
    let context_hash: Option<ContextHash> = to_hash(context_hash.into());
    let block_hash: Option<BlockHash> = to_hash(block_hash.into());
    let operation_hash: Option<OperationHash> = to_hash(operation_hash.into());

    let keyval_and_json: Tuple = keyval_and_json.into();
    let key: ContextKey = List::from(keyval_and_json.get(0).unwrap()).convert_to();
    let value: ContextValue = OcamlBytes::from(keyval_and_json.get(1).unwrap()).convert_to();
    let json_val: Option<String> = to_string(keyval_and_json.get(2).unwrap().into());
    let ignored: bool = keyval_and_json.get(3).unwrap().i32_val() == TRUE.i32_val();

    let time_period: Tuple = time_period.into();
    let start_time: f64 = time_period.get(0).unwrap().f64_val();
    let end_time: f64 = time_period.get(1).unwrap().f64_val();

    context_set(context_hash, block_hash, operation_hash, key, value, json_val, ignored, start_time, end_time);
    return Value::unit();
});

// External callback function for delete key from context
caml!(ml_context_delete(context_hash, block_hash, operation_hash, keyval, time_period) {
    let context_hash: Option<ContextHash> = to_hash(context_hash.into());
    let block_hash: Option<BlockHash> = to_hash(block_hash.into());
    let operation_hash: Option<OperationHash> = to_hash(operation_hash.into());

    let keyval: Tuple = keyval.into();
    let key: ContextKey = List::from(keyval.get(0).unwrap()).convert_to();
    let ignored: bool = keyval.get(1).unwrap().i32_val() == TRUE.i32_val();

    let time_period: Tuple = time_period.into();
    let start_time: f64 = time_period.get(0).unwrap().f64_val();
    let end_time: f64 = time_period.get(1).unwrap().f64_val();

    context_delete(context_hash, block_hash, operation_hash, key, ignored, start_time, end_time);
    return Value::unit();
});

// External callback function for remove_rec key from context
caml!(ml_context_remove_rec(context_hash, block_hash, operation_hash, keyval, time_period) {
    let context_hash: Option<ContextHash> = to_hash(context_hash.into());
    let block_hash: Option<BlockHash> = to_hash(block_hash.into());
    let operation_hash: Option<OperationHash> = to_hash(operation_hash.into());

    let keyval: Tuple = keyval.into();
    let key: ContextKey = List::from(keyval.get(0).unwrap()).convert_to();
    let ignored: bool = keyval.get(1).unwrap().i32_val() == TRUE.i32_val();

    let time_period: Tuple = time_period.into();
    let start_time: f64 = time_period.get(0).unwrap().f64_val();
    let end_time: f64 = time_period.get(1).unwrap().f64_val();

    context_remove_rec(context_hash, block_hash, operation_hash, key, ignored, start_time, end_time);
    return Value::unit();
});

// External callback function for copy keys from context
caml!(ml_context_copy(context_hash, block_hash, operation_hash, from_to_key, time_period) {
    let context_hash: Option<ContextHash> = to_hash(context_hash.into());
    let block_hash: Option<BlockHash> = to_hash(block_hash.into());
    let operation_hash: Option<OperationHash> = to_hash(operation_hash.into());

    let from_to_key: Tuple = from_to_key.into();
    let from_key: ContextKey = List::from(from_to_key.get(0).unwrap()).convert_to();
    let to_key: ContextKey = List::from(from_to_key.get(1).unwrap()).convert_to();
    let ignored: bool = from_to_key.get(2).unwrap().i32_val() == TRUE.i32_val();

    let time_period: Tuple = time_period.into();
    let start_time: f64 = time_period.get(0).unwrap().f64_val();
    let end_time: f64 = time_period.get(1).unwrap().f64_val();

    context_copy(context_hash, block_hash, operation_hash, from_key, to_key, ignored, start_time, end_time);
    return Value::unit();
});

// External callback function for checkout context
caml!(ml_context_checkout(context_hash, time_period) {
    let context_hash: ContextHash = to_hash(context_hash.into()).unwrap();

    let time_period: Tuple = time_period.into();
    let start_time: f64 = time_period.get(0).unwrap().f64_val();
    let end_time: f64 = time_period.get(1).unwrap().f64_val();

    context_checkout(context_hash, start_time, end_time);
    return Value::unit();
});

// External callback function for checkout context
caml!(ml_context_commit(parent_context_hash, block_hash, new_context_hash, time_period) {
    let parent_context_hash: Option<ContextHash> = to_hash(parent_context_hash.into());
    let block_hash: Option<BlockHash> = to_hash(block_hash.into());
    let new_context_hash: ContextHash = to_hash(new_context_hash.into()).unwrap();

    let time_period: Tuple = time_period.into();
    let start_time: f64 = time_period.get(0).unwrap().f64_val();
    let end_time: f64 = time_period.get(1).unwrap().f64_val();

    context_commit(parent_context_hash, block_hash, new_context_hash, start_time, end_time);
    return Value::unit();
});

// External callback function for mem key from context
caml!(ml_context_mem(context_hash, block_hash, operation_hash, key, time_period) {
    let context_hash: Option<ContextHash> = to_hash(context_hash.into());
    let block_hash: Option<BlockHash> = to_hash(block_hash.into());
    let operation_hash: Option<OperationHash> = to_hash(operation_hash.into());
    let key: ContextKey = List::from(key).convert_to();

    let time_period: Tuple = time_period.into();
    let start_time: f64 = time_period.get(0).unwrap().f64_val();
    let end_time: f64 = time_period.get(1).unwrap().f64_val();

    context_mem(context_hash, block_hash, operation_hash, key, start_time, end_time);
    return Value::unit();
});

// External callback function for dir_mem key from context
caml!(ml_context_dir_mem(context_hash, block_hash, operation_hash, key, time_period) {
    let context_hash: Option<ContextHash> = to_hash(context_hash.into());
    let block_hash: Option<BlockHash> = to_hash(block_hash.into());
    let operation_hash: Option<OperationHash> = to_hash(operation_hash.into());
    let key: ContextKey = List::from(key).convert_to();

    let time_period: Tuple = time_period.into();
    let start_time: f64 = time_period.get(0).unwrap().f64_val();
    let end_time: f64 = time_period.get(1).unwrap().f64_val();

    context_dir_mem(context_hash, block_hash, operation_hash, key, start_time, end_time);
    return Value::unit();
});

// External callback function for raw_get key from context
caml!(ml_context_raw_get(context_hash, block_hash, operation_hash, key, time_period) {
    let context_hash: Option<ContextHash> = to_hash(context_hash.into());
    let block_hash: Option<BlockHash> = to_hash(block_hash.into());
    let operation_hash: Option<OperationHash> = to_hash(operation_hash.into());
    let key: ContextKey = List::from(key).convert_to();

    let time_period: Tuple = time_period.into();
    let start_time: f64 = time_period.get(0).unwrap().f64_val();
    let end_time: f64 = time_period.get(1).unwrap().f64_val();

    context_raw_get(context_hash, block_hash, operation_hash, key, start_time, end_time);
    return Value::unit();
});

// External callback function for fold key from context
caml!(ml_context_fold(context_hash, block_hash, operation_hash, key, time_period) {
    let context_hash: Option<ContextHash> = to_hash(context_hash.into());
    let block_hash: Option<BlockHash> = to_hash(block_hash.into());
    let operation_hash: Option<OperationHash> = to_hash(operation_hash.into());
    let key: ContextKey = List::from(key).convert_to();

    let time_period: Tuple = time_period.into();
    let start_time: f64 = time_period.get(0).unwrap().f64_val();
    let end_time: f64 = time_period.get(1).unwrap().f64_val();

    context_fold(context_hash, block_hash, operation_hash, key, start_time, end_time);
    return Value::unit();
});

fn context_set(
    context_hash: Option<ContextHash>,
    block_hash: Option<BlockHash>,
    operation_hash: Option<OperationHash>,
    key: ContextKey,
    value: ContextValue,
    value_as_json: Option<String>,
    ignored: bool,
    start_time: f64,
    end_time: f64)
{
    context_send(ContextAction::Set {
        context_hash,
        block_hash,
        operation_hash,
        key,
        value,
        value_as_json,
        ignored,
        start_time,
        end_time,
    }).expect("context_set error");
}

fn context_delete(
    context_hash: Option<ContextHash>,
    block_hash: Option<BlockHash>,
    operation_hash: Option<OperationHash>,
    key: ContextKey,
    ignored: bool,
    start_time: f64,
    end_time: f64)
{
    context_send(ContextAction::Delete {
        context_hash,
        block_hash,
        operation_hash,
        key,
        ignored,
        start_time,
        end_time,
    }).expect("context_delete error");
}

fn context_remove_rec(
    context_hash: Option<ContextHash>,
    block_hash: Option<BlockHash>,
    operation_hash: Option<OperationHash>,
    key: ContextKey,
    ignored: bool,
    start_time: f64,
    end_time: f64)
{
    context_send(ContextAction::RemoveRecursively {
        context_hash,
        block_hash,
        operation_hash,
        key,
        ignored,
        start_time,
        end_time,
    }).expect("context_remove_rec error");
}

fn context_copy(
    context_hash: Option<ContextHash>,
    block_hash: Option<BlockHash>,
    operation_hash: Option<OperationHash>,
    from_key: ContextKey,
    to_key: ContextKey,
    ignored: bool,
    start_time: f64,
    end_time: f64)
{
    context_send(ContextAction::Copy {
        context_hash,
        block_hash,
        operation_hash,
        from_key,
        to_key,
        ignored,
        start_time,
        end_time,
    }).expect("context_copy error");
}

fn context_checkout(
    context_hash: ContextHash,
    start_time: f64,
    end_time: f64)
{
    context_send(ContextAction::Checkout {
        context_hash,
        start_time,
        end_time,
    }).expect("context_checkout error");
}

fn context_commit(
    parent_context_hash: Option<ContextHash>,
    block_hash: Option<BlockHash>,
    new_context_hash: ContextHash,
    start_time: f64,
    end_time: f64)
{
    context_send(ContextAction::Commit {
        parent_context_hash,
        block_hash,
        new_context_hash,
        start_time,
        end_time,
    }).expect("context_commit error");
}

fn context_mem(
    context_hash: Option<ContextHash>,
    block_hash: Option<BlockHash>,
    operation_hash: Option<OperationHash>,
    key: ContextKey,
    start_time: f64,
    end_time: f64)
{
    context_send(ContextAction::Mem {
        context_hash,
        block_hash,
        operation_hash,
        key,
        start_time,
        end_time,
    }).expect("context_mem error");
}

fn context_dir_mem(
    context_hash: Option<ContextHash>,
    block_hash: Option<BlockHash>,
    operation_hash: Option<OperationHash>,
    key: ContextKey,
    start_time: f64,
    end_time: f64)
{
    context_send(ContextAction::DirMem {
        context_hash,
        block_hash,
        operation_hash,
        key,
        start_time,
        end_time,
    }).expect("context_dir_mem error");
}

fn context_raw_get(
    context_hash: Option<ContextHash>,
    block_hash: Option<BlockHash>,
    operation_hash: Option<OperationHash>,
    key: ContextKey,
    start_time: f64,
    end_time: f64)
{
    context_send(ContextAction::Get {
        context_hash,
        block_hash,
        operation_hash,
        key,
        start_time,
        end_time,
    }).expect("context_get error");
}

fn context_fold(
    context_hash: Option<ContextHash>,
    block_hash: Option<BlockHash>,
    operation_hash: Option<OperationHash>,
    key: ContextKey,
    start_time: f64,
    end_time: f64)
{
    context_send(ContextAction::Fold {
        context_hash,
        block_hash,
        operation_hash,
        key,
        start_time,
        end_time,
    }).expect("context_fold error");
}