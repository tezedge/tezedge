// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crossbeam_channel::{Receiver, Sender, unbounded};
use crypto::hash::{BlockHash, ContextHash, OperationHash};
use ocaml_interop::*;
use tezos_api::ocaml_conv::{OCamlBlockHash, OCamlContextHash, OCamlOperationHash};
use once_cell::sync::Lazy;

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
    let name: String = action_name.to_rust(rt);
    let key: Vec<String> = key.to_rust(rt);
    // let _ = (action_name, key, irmin_time, tezedge_time);
    // TODO

    let action = Action {
        name,
        key,
        irmin_time,
        tezedge_time,
    };

    TIMING_CHANNEL
        .send(TimingMessage::Action(action))
        .unwrap();
}

// TODO: add tree_action

enum TimingMessage {
    SetBlock(Option<BlockHash>),
    SetOperation(Option<OperationHash>),
    Action(Action),
}

#[derive(Default)]
struct Timing {
    current_block: Option<BlockHash>,
    current_operation: Option<OperationHash>,
    blocks: Vec<Block>,
}

struct Block {
    hash: BlockHash,
    operations: Vec<Operation>
}

struct Operation {
    hash: OperationHash,
    actions: Vec<Action>
}

struct Action {
    name: String,
    key: Vec<String>,
    irmin_time: f64,
    tezedge_time: f64,
}

static TIMING_CHANNEL: Lazy<Sender<TimingMessage>> = Lazy::new(|| {
    let (sender, receiver) = unbounded();

    std::thread::Builder::new()
        .name("timing".to_string())
        .spawn(|| {
            start_timing(receiver);
        })
        .unwrap();

    sender
});

fn start_timing(recv: Receiver<TimingMessage>) {
    let mut timing = Timing::default();

    for msg in recv {
        timing.process_msg(msg);
    }
}

impl Timing {
    fn process_msg(&mut self, msg: TimingMessage) {
        match msg {
            TimingMessage::SetBlock(block_hash) => {
                self.current_block = block_hash;
            }
            TimingMessage::SetOperation(operation_hash) => {
                self.current_operation = operation_hash;
            }
            TimingMessage::Action(action) => {

            }
        }
    }
}
