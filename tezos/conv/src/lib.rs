// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{BlockHash, ContextHash, Hash, OperationListListHash};
use tezos_messages::p2p::encoding::{block_header::Level, fitness::Fitness};

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
pub struct OCamlBlockPayloadHash {}
pub struct OCamlOperationMetadataHash {}
pub struct OCamlOperationMetadataListListHash {}
pub struct OCamlChainId {}

// Headers, operations and other

pub struct OCamlBlockHeaderShellHeader {}
pub struct OCamlBlockHeader {}
pub struct OCamlOperationShellHeader {}
pub struct OCamlOperation {}
pub struct OCamlGenesisChain {}
pub struct OCamlProtocolOverrides {}
pub struct OCamlPatchContext {}
pub struct OCamlTezosRuntimeConfiguration {}
pub struct OCamlTezosRuntimeLogLevel {}
pub struct OCamlJsonEncodeApplyBlockResultMetadataParams {}
pub struct OCamlJsonEncodeApplyBlockOperationsMetadataParams {}

// Context

pub struct OCamlTezosContextIrminStorageConfiguration {}
pub struct OCamlTezosContextStorageConfiguration {}
pub struct OCamlTezosContextConfiguration {}
pub struct OCamlContextGetKeyFromHistoryRequest {}
pub struct OCamlContextGetKeyValuesByPrefixRequest {}
pub struct OCamlContextGetTreeByPrefixRequest {}

// Dumps
pub struct OCamlDumpContextRequest {}
pub struct OCamlRestoreContextRequest {}

// Requests

pub struct OCamlApplyBlockRequest {}
pub struct OCamlBeginApplicationRequest {}
pub struct OCamlBeginConstructionRequest {}
pub struct OCamlValidateOperationRequest {}

pub struct OCamlRpcRequest {}
pub struct OCamlProtocolRpcRequest {}
pub struct OCamlHelpersPreapplyBlockRequest {}
pub struct OCamlComputePathRequest {}
pub struct OCamlGenesisResultDataParams {}
pub struct OCamlInitProtocolContextParams {}

// Responses

pub struct OCamlApplied {}
pub struct OCamlApplyBlockResponse {}
pub struct OCamlApplyBlockExecutionTimestamps {}
pub struct OCamlBeginApplicationResponse {}
pub struct OCamlTezosContextTezedgeOnDiskBackendOptions {}
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
pub struct OCamlInitProtocolContextResult {}
pub struct OCamlCommitGenesisResult {}
pub struct OCamlComputePathResponse {}

// Other

pub struct OCamlCycleRollsOwnerSnapshot {}

// IPC messages
pub struct OCamlProtocolMessage {}
pub struct OCamlNodeMessage {}

pub mod from_ocaml;
pub mod to_ocaml;

pub fn force_libtezos_linking() {
    tezos_sys::force_libtezos_linking();
}
