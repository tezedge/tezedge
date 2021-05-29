// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{BlockHash, ContextHash, OperationHash};
use ocaml_interop::*;
use tezos_api::ocaml_conv::{OCamlBlockHash, OCamlContextHash, OCamlOperationHash};
use tezos_timing::{Action, ActionKind, TimingMessage, TIMING_CHANNEL};

pub fn set_block(rt: &OCamlRuntime, block_hash: OCamlRef<Option<OCamlBlockHash>>) {
    let block_hash: Option<BlockHash> = block_hash.to_rust(rt);

    TIMING_CHANNEL
        .send(TimingMessage::SetBlock(block_hash))
        .unwrap();
}

pub fn set_operation(rt: &OCamlRuntime, operation_hash: OCamlRef<Option<OCamlOperationHash>>) {
    let operation_hash: Option<OperationHash> = operation_hash.to_rust(rt);

    TIMING_CHANNEL
        .send(TimingMessage::SetOperation(operation_hash))
        .unwrap();
}

pub fn checkout(
    rt: &OCamlRuntime,
    context_hash: OCamlRef<OCamlContextHash>,
    irmin_time: f64,
    tezedge_time: f64,
) {
    let context_hash: ContextHash = context_hash.to_rust(rt);
    let irmin_time = match irmin_time {
        t if t < 0.0 => None,
        t => Some(t)
    };
    let tezedge_time = match tezedge_time {
        t if t < 0.0 => None,
        t => Some(t)
    };

    TIMING_CHANNEL
        .send(TimingMessage::Checkout {
            context_hash,
            irmin_time,
            tezedge_time,
        })
        .unwrap();
}

pub fn commit(
    _rt: &OCamlRuntime,
    _new_context_hash: OCamlRef<OCamlContextHash>,
    irmin_time: f64,
    tezedge_time: f64,
) {
    let irmin_time = match irmin_time {
        t if t < 0.0 => None,
        t => Some(t)
    };
    let tezedge_time = match tezedge_time {
        t if t < 0.0 => None,
        t => Some(t)
    };

    TIMING_CHANNEL
        .send(TimingMessage::Commit {
            irmin_time,
            tezedge_time,
        })
        .unwrap();
}

pub fn context_action(
    rt: &OCamlRuntime,
    action_name: OCamlRef<String>,
    key: OCamlRef<OCamlList<String>>,
    irmin_time: f64,
    tezedge_time: f64,
) {
    let action_name = rt.get(action_name);
    let action_name = match action_name.as_bytes() {
        b"mem" => ActionKind::Mem,
        b"find" => ActionKind::Find,
        b"find_tree" => ActionKind::FindTree,
        b"add" => ActionKind::Add,
        b"add_tree" => ActionKind::AddTree,
        b"remove" => ActionKind::Remove,
        bytes => {
            println!("ERROR={:?}", std::str::from_utf8(bytes));
            return;
        }
    };
    let irmin_time = match irmin_time {
        t if t < 0.0 => None,
        t => Some(t)
    };
    let tezedge_time = match tezedge_time {
        t if t < 0.0 => None,
        t => Some(t)
    };

    let key: Vec<String> = key.to_rust(rt);

    let action = Action {
        action_name,
        key,
        irmin_time,
        tezedge_time,
    };

    TIMING_CHANNEL.send(TimingMessage::Action(action)).unwrap();
}

pub fn init_timing(db_path: String) {
    TIMING_CHANNEL
        .send(TimingMessage::InitTiming {
            db_path: Some(db_path.into()),
        })
        .unwrap();
}
