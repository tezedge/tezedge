// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ocaml_interop::*;
use tezos_conv::{
    from_ocaml::hash_as_bytes, OCamlBlockHash, OCamlContextHash, OCamlOperationHash,
    OCamlProtocolHash,
};
use tezos_timing::{
    container::{
        InlinedBlockHash, InlinedContextHash, InlinedOperationHash, InlinedProtocolHash,
        InlinedString,
    },
    BlockMemoryUsage, ChannelError, Query, TimingMessage, TIMING_CHANNEL,
};

use crate::from_ocaml::OCamlQueryKind;

fn send_msg(msg: TimingMessage) -> Result<(), ChannelError> {
    TIMING_CHANNEL.with(|channel| {
        let mut channel = channel.borrow_mut();
        channel.send(msg)
    })
}

pub fn send_statistics(stats: BlockMemoryUsage) {
    if let Err(e) = send_msg(TimingMessage::BlockMemoryUsage { stats }) {
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

    let timestamp = if block_hash.is_some() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::new(0, 0));
        Some(timestamp)
    } else {
        None
    };

    if let Err(e) = send_msg(TimingMessage::SetBlock {
        block_hash,
        timestamp,
        instant,
    }) {
        eprintln!("Timing set_block hook error = {:?}", e);
    }
}

pub fn set_protocol(rt: &OCamlRuntime, protocol_hash: OCamlRef<Option<OCamlProtocolHash>>) {
    let hash = rt.get(protocol_hash);

    let protocol_hash = hash.to_option().map(|h| {
        let bytes = hash_as_bytes(h);
        InlinedProtocolHash::from(bytes)
    });

    if let Err(e) = send_msg(TimingMessage::SetProtocol(protocol_hash)) {
        eprintln!("Timing set_block hook error = {:?}", e);
    }
}

pub fn set_operation(rt: &OCamlRuntime, operation_hash: OCamlRef<Option<OCamlOperationHash>>) {
    let hash = rt.get(operation_hash);

    let operation_hash = hash.to_option().map(|h| {
        let bytes = hash_as_bytes(h);
        InlinedOperationHash::from(bytes)
    });

    if let Err(e) = send_msg(TimingMessage::SetOperation(operation_hash)) {
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

    let irmin_time = get_time(irmin_time);
    let tezedge_time = get_time(tezedge_time);

    if let Err(e) = send_msg(TimingMessage::Checkout {
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
    let irmin_time = get_time(irmin_time);
    let tezedge_time = get_time(tezedge_time);

    if let Err(e) = send_msg(TimingMessage::Commit {
        irmin_time,
        tezedge_time,
    }) {
        eprintln!("Timing commit hook error = {:?}", e);
    }
}

pub fn context_query(
    rt: &OCamlRuntime,
    query_kind: OCamlRef<OCamlQueryKind>,
    key: OCamlRef<OCamlList<String>>,
    irmin_time: f64,
    tezedge_time: f64,
) {
    let query_kind = query_kind.to_rust(rt);

    let irmin_time = get_time(irmin_time);
    let tezedge_time = get_time(tezedge_time);

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

    let query = Query {
        query_kind,
        key: string,
        irmin_time,
        tezedge_time,
    };

    if let Err(e) = send_msg(TimingMessage::Query(query)) {
        eprintln!("Timing context_query hook error = {:?}", e);
    }
}

pub fn init_timing(db_path: String) {
    if let Err(e) = send_msg(TimingMessage::InitTiming {
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
