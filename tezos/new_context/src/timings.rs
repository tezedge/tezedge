// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{BlockHash, ContextHash, OperationHash};
use ocaml_interop::*;
use tezos_api::ocaml_conv::{OCamlBlockHash, OCamlContextHash, OCamlOperationHash};

pub fn set_block(rt: &OCamlRuntime, block_hash: OCamlRef<Option<OCamlBlockHash>>) {
    let block_hash: Option<BlockHash> = block_hash.to_rust(rt);
    let _ = block_hash;
    // TODO
}

pub fn set_operation(rt: &OCamlRuntime, operation_hash: OCamlRef<Option<OCamlOperationHash>>) {
    let operation_hash: Option<OperationHash> = operation_hash.to_rust(rt);
    let _ = operation_hash;
    // TODO
}

pub fn checkout(
    rt: &OCamlRuntime,
    context_hash: OCamlRef<OCamlContextHash>,
    irmin_time: f64,
    tezedge_time: f64,
) {
    let context_hash: ContextHash = context_hash.to_rust(rt);
    let _ = (context_hash, irmin_time, tezedge_time);
    // TODO
}

pub fn commit(
    rt: &OCamlRuntime,
    new_context_hash: OCamlRef<OCamlContextHash>,
    irmin_time: f64,
    tezedge_time: f64,
) {
    let new_context_hash: ContextHash = new_context_hash.to_rust(rt);
    let _ = (new_context_hash, irmin_time, tezedge_time);
    // TODO
}

pub fn context_action(
    rt: &OCamlRuntime,
    action_name: OCamlRef<String>,
    key: OCamlRef<OCamlList<String>>,
    irmin_time: f64,
    tezedge_time: f64,
) {
    // TODO - Bruno: it is possible to avoid these conversions by borrowing the internal
    // &str directly. Since we want this function to add as little overhead as possible
    // investigate doing that later once everything is working properly.
    let action_name: String = action_name.to_rust(rt);
    let key: Vec<String> = key.to_rust(rt);
    let _ = (action_name, key, irmin_time, tezedge_time);
    // TODO
}

// TODO: add tree_action