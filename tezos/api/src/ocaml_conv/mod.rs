// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::encoding::{
    block_header::BlockHeader, operation::Operation, operations_for_blocks::Path,
};

// FFI Wrappers
#[repr(transparent)]
pub struct FfiPath(pub Path);
#[repr(transparent)]
pub struct FfiBlockHeader(pub BlockHeader);
#[repr(transparent)]
pub struct FfiOperation(pub Operation);

// Hashes
struct OCamlHash {}
struct OCamlOperationListListHash {}
pub struct OCamlOperationHash {}
struct OCamlBlockHash {}
struct OCamlContextHash {}
struct OCamlProtocolHash {}

pub mod from_ocaml;
pub mod to_ocaml;
