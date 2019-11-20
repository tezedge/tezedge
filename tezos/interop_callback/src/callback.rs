// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use ocaml::{Array1, caml, List, Str, Value};

use tezos_context::*;
use tezos_context::channel::*;

pub type OcamlBytes = Array1<u8>;

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

fn to_hash(hash: OcamlBytes) -> Option<Hash> {
    if hash.is_empty() {
        None
    } else {
        Some(hash.convert_to())
    }
}

// External callback function for set value to context
caml!(ml_context_set(context_hash, block_hash, operation_hash, key, value) {
    let context_hash: Option<ContextHash> = to_hash(context_hash.into());
    let block_hash: Option<BlockHash> = to_hash(block_hash.into());
    let operation_hash: Option<OperationHash> = to_hash(operation_hash.into());
    let key: ContextKey = List::from(key).convert_to();
    let value: ContextValue = OcamlBytes::from(value).convert_to();
    context_set(context_hash, block_hash, operation_hash, key, value);
    return Value::unit();
});

// External callback function for delete key from context
caml!(ml_context_delete(context_hash, block_hash, operation_hash, key) {
    let context_hash: Option<ContextHash> = to_hash(context_hash.into());
    let block_hash: Option<BlockHash> = to_hash(block_hash.into());
    let operation_hash: Option<OperationHash> = to_hash(operation_hash.into());
    let key: ContextKey = List::from(key).convert_to();
    context_delete(context_hash, block_hash, operation_hash, key);
    return Value::unit();
});

// External callback function for remove_rec key from context
caml!(ml_context_remove_rec(context_hash, block_hash, operation_hash, key) {
    let context_hash: Option<ContextHash> = to_hash(context_hash.into());
    let block_hash: Option<BlockHash> = to_hash(block_hash.into());
    let operation_hash: Option<OperationHash> = to_hash(operation_hash.into());
    let key: ContextKey = List::from(key).convert_to();
    context_remove_rec(context_hash, block_hash, operation_hash, key);
    return Value::unit();
});

// External callback function for copy keys from context
caml!(ml_context_copy(context_hash, block_hash, operation_hash, from_key, to_key) {
    let context_hash: Option<ContextHash> = to_hash(context_hash.into());
    let block_hash: Option<BlockHash> = to_hash(block_hash.into());
    let operation_hash: Option<OperationHash> = to_hash(operation_hash.into());
    let from_key: ContextKey = List::from(from_key).convert_to();
    let to_key: ContextKey = List::from(to_key).convert_to();
    context_copy(context_hash, block_hash, operation_hash, from_key, to_key);
    return Value::unit();
});

// External callback function for checkout context
caml!(ml_context_checkout(context_hash) {
    let context_hash: ContextHash = OcamlBytes::from(context_hash).convert_to();
    context_checkout(context_hash);
    return Value::unit();
});

// External callback function for checkout context
caml!(ml_context_commit(parent_context_hash, block_hash, new_context_hash) {
    let parent_context_hash: Option<ContextHash> = to_hash(parent_context_hash.into());
    let block_hash: Option<BlockHash> = to_hash(block_hash.into());
    let new_context_hash: ContextHash = OcamlBytes::from(new_context_hash).convert_to();
    context_commit(parent_context_hash, block_hash, new_context_hash);
    return Value::unit();
});

pub fn context_set(context_hash: Option<ContextHash>, block_hash: Option<BlockHash>, operation_hash: Option<OperationHash>, key: ContextKey, value: ContextValue) {
    context_send(ContextAction::Set { context_hash, block_hash, operation_hash, key, value }).unwrap();
}

pub fn context_delete(context_hash: Option<ContextHash>, block_hash: Option<BlockHash>, operation_hash: Option<OperationHash>, key: ContextKey) {
    context_send(ContextAction::Delete { context_hash, block_hash, operation_hash, key }).unwrap();
}

pub fn context_remove_rec(context_hash: Option<ContextHash>, block_hash: Option<BlockHash>, operation_hash: Option<OperationHash>, key: ContextKey) {
    context_send(ContextAction::RemoveRecord { context_hash, block_hash, operation_hash, key }).unwrap();
}

pub fn context_copy(context_hash: Option<ContextHash>, block_hash: Option<BlockHash>, operation_hash: Option<OperationHash>, from_key: ContextKey, to_key: ContextKey) {
    context_send(ContextAction::Copy { context_hash, block_hash, operation_hash, from_key, to_key }).unwrap();
}

pub fn context_checkout(context_hash: ContextHash) {
    context_send(ContextAction::Checkout { context_hash }).unwrap();
}

pub fn context_commit(parent_context_hash: Option<ContextHash>, block_hash: Option<BlockHash>, new_context_hash: ContextHash) {
    context_send(ContextAction::Commit { parent_context_hash, block_hash, new_context_hash }).unwrap();
}