// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ocaml_interop::*;
use tezos_api::ocaml_conv::{OCamlBlockHash, OCamlContextHash, OCamlOperationHash, from_ocaml::hash_as_bytes};
use tezos_timing::{Action, QueryKind, BlockMemoryUsage, TIMING_CHANNEL, TimingMessage, container::{InlinedString, InlinedBlockHash, InlinedContextHash, InlinedOperationHash}};

pub fn send_statistics(stats: BlockMemoryUsage) {
    // return;

    if let Err(e) = TIMING_CHANNEL.send(TimingMessage::BlockMemoryUsage { stats }) {
        eprintln!("send_statistics error = {:?}", e);
    }
}

pub fn set_block(rt: &OCamlRuntime, block_hash: OCamlRef<Option<OCamlBlockHash>>) {
    let hash = rt.get(block_hash);

    let block_hash = hash.to_option().map(|h| {
        let bytes = hash_as_bytes(h);
        InlinedBlockHash::from(bytes)
    });

    let instant = Instant::now();
    // let block_hash: Option<BlockHash> = block_hash.to_rust(rt);

    let timestamp = if block_hash.is_some() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::new(0, 0));
        Some(timestamp)
    } else {
        None
    };

    if let Err(e) = TIMING_CHANNEL.send(TimingMessage::SetBlock {
        block_hash,
        timestamp,
        instant,
    }) {
        eprintln!("Timing set_block hook error = {:?}", e);
    }
}

pub fn set_operation(rt: &OCamlRuntime, operation_hash: OCamlRef<Option<OCamlOperationHash>>) {
    let hash = rt.get(operation_hash);

    let operation_hash = hash.to_option().map(|h| {
        let bytes = hash_as_bytes(h);
        InlinedOperationHash::from(bytes)
    });

    // let operation_hash: Option<OperationHash> = operation_hash.to_rust(rt);

    if let Err(e) = TIMING_CHANNEL.send(TimingMessage::SetOperation(operation_hash)) {
        eprintln!("Timing set_operation hook error = {:?}", e);
    }
}

pub fn checkout(
    rt: &OCamlRuntime,
    context_hash: OCamlRef<OCamlContextHash>,
    irmin_time: f64,
    tezedge_time: f64,
) {
    let hash = rt.get(context_hash);
    let bytes = hash_as_bytes(hash);
    let context_hash = InlinedContextHash::from(bytes);

    // println!("CHECKOUT LEN={:?}", bytes.len());

    // return;

    // let context_hash: ContextHash = context_hash.to_rust(rt);
    let irmin_time = get_time(irmin_time);
    let tezedge_time = get_time(tezedge_time);

    if let Err(e) = TIMING_CHANNEL.send(TimingMessage::Checkout {
        context_hash,
        irmin_time,
        tezedge_time,
    }) {
        eprintln!("Timing checkout hook error = {:?}", e);
    }
}

pub fn commit(
    _rt: &OCamlRuntime,
    _new_context_hash: OCamlRef<OCamlContextHash>,
    irmin_time: f64,
    tezedge_time: f64,
) {
    // return;

    let irmin_time = get_time(irmin_time);
    let tezedge_time = get_time(tezedge_time);

    if let Err(e) = TIMING_CHANNEL.send(TimingMessage::Commit {
        irmin_time,
        tezedge_time,
    }) {
        eprintln!("Timing commit hook error = {:?}", e);
    }
}

fn get_action_kind(name: &[u8]) -> Option<ActionKind> {
    let first = name.get(0)?;
    let length = name.len();

    match first {
        b'm' => {
            if length == 3 {
                Some(ActionKind::Mem)
            } else {
                Some(ActionKind::MemTree)
            }
        }
        b'f' => {
            if length == 3 {
                Some(ActionKind::Find)
            } else {
                Some(ActionKind::FindTree)
            }
        }
        b'a' => {
            if length == 3 {
                Some(ActionKind::Add)
            } else {
                Some(ActionKind::AddTree)
            }
        }
        b'r' => Some(ActionKind::Remove),
        _ => None
    }
}

pub fn context_action(
    rt: &OCamlRuntime,
    query_name: OCamlRef<String>,
    key: OCamlRef<OCamlList<String>>,
    irmin_time: f64,
    tezedge_time: f64,
) {
    // return;

    let action_name = rt.get(action_name);
    // let action_name = match action_name.as_bytes() {
    //     b"mem" => ActionKind::Mem,
    //     b"mem_tree" => ActionKind::MemTree,
    //     b"find" => ActionKind::Find,
    //     b"find_tree" => ActionKind::FindTree,
    //     b"add" => ActionKind::Add,
    //     b"add_tree" => ActionKind::AddTree,
    //     b"remove" => ActionKind::Remove,
    //     _ => return,
    // };

    let action_name = match get_action_kind(action_name.as_bytes()) {
        Some(name) => name,
        None => return,
    };

    // let action_name = ActionKind::Remove;

    let irmin_time = get_time(irmin_time);
    let tezedge_time = get_time(tezedge_time);


    // let vector = String::new();

    let mut key = rt.get(key);
    let mut string: InlinedString = Default::default();

    let mut first = true;

    while let Some((head, tail)) = key.uncons() {
        if first {
            first = false;
        } else {
            string.push_str("/");
        }
        string.push_str(unsafe { head.as_str_unchecked() });
        key = tail;
    }

    // println!("LEN={:?}", string.len());

    // vector

    // let key: Vec<String> = key.to_rust(rt);

    let action = Action {
        action_name,
        key: string,
        irmin_time,
        tezedge_time,
    };

    if let Err(e) = TIMING_CHANNEL.send(TimingMessage::Query(query)) {
        eprintln!("Timing context_query hook error = {:?}", e);
    }
}

pub fn init_timing(db_path: String) {
    if let Err(e) = TIMING_CHANNEL.send(TimingMessage::InitTiming {
        db_path: Some(db_path.into()),
    }) {
        eprintln!("Timing init_timing hook error = {:?}", e);
    }
}

fn get_time(time: f64) -> Option<f64> {
    match time {
        t if t < 0.0 => None,
        t => Some(t),
    }
}
