// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::Hash;
use tezos_messages::p2p::encoding::{
    block_header::{Fitness, Level},
    operations_for_blocks::Path,
};

// FFI Wrappers:
// Defined to be able to implement ToOCaml/FromOCaml traits for
// structs that are defined in another crate and/or do not map directly
// to the matching OCaml struct.

pub struct FfiPath(pub Path);
pub struct FfiBlockHeader<'a> {
    shell: FfiBlockHeaderShellHeader<'a>,
    protocol_data: &'a Vec<u8>,
}
pub struct FfiBlockHeaderShellHeader<'a> {
    level: Level,
    proto_level: i32,
    predecessor: TaggedHash<'a>,
    timestamp: i64,
    validation_passes: i32,
    operations_hash: TaggedHash<'a>,
    fitness: &'a Fitness,
    context: TaggedHash<'a>,
}
pub struct FfiOperation<'a> {
    shell: FfiOperationShellHeader<'a>,
    data: &'a Vec<u8>,
}
pub struct FfiOperationShellHeader<'a> {
    pub branch: TaggedHash<'a>,
}

pub struct TaggedHash<'a>(pub &'a Hash);

// Hashes
pub struct OCamlHash {}
pub struct OCamlOperationListListHash {}
pub struct OCamlOperationHash {}
pub struct OCamlBlockHash {}
pub struct OCamlContextHash {}
pub struct OCamlProtocolHash {}

pub mod from_ocaml;
pub mod to_ocaml;
