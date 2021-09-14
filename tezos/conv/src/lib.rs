// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{BlockHash, ContextHash, Hash, OperationListListHash};
use tezos_messages::p2p::encoding::block_header::{Fitness, Level};

// FFI Wrappers:
// Defined to be able to implement ToOCaml/FromOCaml traits for
// structs that are defined in another crate and/or do not map directly
// to the matching OCaml struct.

pub struct FfiPathRight {
    pub left: Hash,
    pub path: FfiPath,
}

pub struct FfiPathLeft {
    pub path: FfiPath,
    pub right: Hash,
}

pub enum FfiPath {
    Right(Box<FfiPathRight>),
    Left(Box<FfiPathLeft>),
    Op,
}

pub struct FfiBlockHeader<'a> {
    shell: FfiBlockHeaderShellHeader<'a>,
    protocol_data: &'a Vec<u8>,
}

pub struct FfiBlockHeaderShellHeader<'a> {
    level: Level,
    proto_level: i32,
    predecessor: &'a BlockHash,
    timestamp: i64,
    validation_passes: i32,
    operations_hash: &'a OperationListListHash,
    fitness: &'a Fitness,
    context: &'a ContextHash,
}

pub struct FfiOperation<'a> {
    shell: FfiOperationShellHeader<'a>,
    data: &'a Vec<u8>,
}

pub struct FfiOperationShellHeader<'a> {
    pub branch: &'a BlockHash,
}

// A hash that is represented as a tagged block in OCaml (borrows form a `Hash`)
pub enum TaggedHash<'a> {
    Hash(&'a Hash),
}

// Hashes

pub struct OCamlHash {}
pub struct OCamlOperationListListHash {}
pub struct OCamlOperationHash {}
pub struct OCamlBlockHash {}
pub struct OCamlContextHash {}
pub struct OCamlProtocolHash {}
pub struct OCamlBlockMetadataHash {}
pub struct OCamlOperationMetadataHash {}
pub struct OCamlOperationMetadataListListHash {}
pub struct OCamlChainId {}

// Headers and operations

pub struct OCamlBlockHeaderShellHeader {}
pub struct OCamlBlockHeader {}
pub struct OCamlOperationShellHeader {}
pub struct OCamlOperation {}

// Context

pub struct OCamlTezosContextIrminStorageConfiguration {}
pub struct OCamlTezosContextStorageConfiguration {}
pub struct OCamlTezosContextConfiguration {}

// Requests

pub struct OCamlApplyBlockRequest {}
pub struct OCamlBeginApplicationRequest {}
pub struct OCamlBeginConstructionRequest {}
pub struct OCamlValidateOperationRequest {}
pub struct OCamlRpcRequest {}
pub struct OCamlProtocolRpcRequest {}
pub struct OCamlHelpersPreapplyBlockRequest {}

// Responses

pub struct OCamlApplied {}
pub struct OCamlApplyBlockResponse {}
pub struct OCamlBeginApplicationResponse {}
pub struct OCamlContextKvStoreConfiguration {}
pub struct OCamlErrored {}
pub struct OCamlForkingTestchainData {}
pub struct OCamlHelpersPreapplyResponse {}
pub struct OCamlOperationProtocolDataJsonWithErrorListJson {}
pub struct OCamlPrevalidatorWrapper {}
pub struct OCamlProtocolRpcError {}
pub struct OCamlProtocolRpcResponse {}
pub struct OCamlRpcArgDesc {}
pub struct OCamlRpcMethod {}
pub struct OCamlTezosContextTezEdgeStorageConfiguration {}
pub struct OCamlTezosErrorTrace {}
pub struct OCamlValidateOperationResponse {}
pub struct OCamlValidateOperationResult {}

// Other

pub struct OCamlCycleRollsOwnerSnapshot {}

pub mod from_ocaml;
pub mod to_ocaml;
