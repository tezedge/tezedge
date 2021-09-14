// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;

use crate::OCamlCycleRollsOwnerSnapshot;

use super::{
    FfiPath, FfiPathLeft, FfiPathRight, OCamlApplied, OCamlApplyBlockResponse,
    OCamlBeginApplicationResponse, OCamlBlockHash, OCamlBlockMetadataHash, OCamlChainId,
    OCamlContextHash, OCamlContextKvStoreConfiguration, OCamlErrored, OCamlForkingTestchainData,
    OCamlHash, OCamlHelpersPreapplyResponse, OCamlOperationHash, OCamlOperationMetadataHash,
    OCamlOperationMetadataListListHash, OCamlOperationProtocolDataJsonWithErrorListJson,
    OCamlPrevalidatorWrapper, OCamlProtocolHash, OCamlProtocolRpcError, OCamlProtocolRpcResponse,
    OCamlRpcArgDesc, OCamlRpcMethod, OCamlTezosContextTezEdgeStorageConfiguration,
    OCamlTezosErrorTrace, OCamlValidateOperationResponse, OCamlValidateOperationResult,
};
use crypto::hash::{
    BlockHash, BlockMetadataHash, ChainId, ContextHash, Hash, OperationHash, OperationMetadataHash,
    OperationMetadataListListHash, ProtocolHash,
};
use ocaml_interop::{
    impl_from_ocaml_record, impl_from_ocaml_variant, FromOCaml, OCaml, OCamlBytes, OCamlFloat,
    OCamlInt, OCamlInt32, OCamlList,
};
use tezos_api::ffi::{
    Applied, ApplyBlockResponse, BeginApplicationResponse, CycleRollsOwnerSnapshot, Errored,
    ForkingTestchainData, HelpersPreapplyResponse, OperationProtocolDataJsonWithErrorListJson,
    PrevalidatorWrapper, ProtocolRpcError, ProtocolRpcResponse, RpcArgDesc, RpcMethod,
    TezosErrorTrace, ValidateOperationResponse, ValidateOperationResult,
};
use tezos_context_api::{ContextKvStoreConfiguration, TezosContextTezEdgeStorageConfiguration};

macro_rules! from_ocaml_hash {
    ($ocaml_name:ident, $rust_name:ident) => {
        unsafe impl FromOCaml<$ocaml_name> for $rust_name {
            fn from_ocaml(v: OCaml<$ocaml_name>) -> Self {
                unsafe { v.field::<OCamlBytes>(0).to_rust() }
            }
        }
    };
}

macro_rules! from_ocaml_typed_hash {
    ($ocaml_name:ident, $rust_name:ident) => {
        unsafe impl FromOCaml<$ocaml_name> for $rust_name {
            fn from_ocaml(v: OCaml<$ocaml_name>) -> Self {
                unsafe {
                    let vec: Vec<u8> = v.field::<OCamlBytes>(0).to_rust();
                    use std::convert::TryFrom;
                    $rust_name::try_from(vec).unwrap_or_else(|e| {
                        unreachable!(format!("Wrong bytes received from OCaml: {:?}", e))
                    })
                }
            }
        }
    };
}

from_ocaml_hash!(OCamlHash, Hash);
from_ocaml_typed_hash!(OCamlOperationHash, OperationHash);
from_ocaml_typed_hash!(OCamlBlockHash, BlockHash);
from_ocaml_typed_hash!(OCamlContextHash, ContextHash);
from_ocaml_typed_hash!(OCamlProtocolHash, ProtocolHash);
from_ocaml_typed_hash!(OCamlBlockMetadataHash, BlockMetadataHash);
from_ocaml_typed_hash!(OCamlOperationMetadataHash, OperationMetadataHash);
from_ocaml_typed_hash!(
    OCamlOperationMetadataListListHash,
    OperationMetadataListListHash
);

// TODO: TE-367: review once ocaml-interop has been upgraded
unsafe impl FromOCaml<OCamlChainId> for ChainId {
    fn from_ocaml(v: OCaml<OCamlChainId>) -> Self {
        let v: OCaml<OCamlBytes> = unsafe { std::mem::transmute(v) };
        let vec: Vec<u8> = v.to_rust();
        ChainId::try_from(vec).unwrap()
    }
}

impl_from_ocaml_variant! {
    OCamlContextKvStoreConfiguration => ContextKvStoreConfiguration {
        ContextKvStoreConfiguration::ReadOnlyIpc,
        ContextKvStoreConfiguration::InMem,
    }
}

impl_from_ocaml_record! {
    OCamlTezosContextTezEdgeStorageConfiguration => TezosContextTezEdgeStorageConfiguration {
        backend: OCamlContextKvStoreConfiguration,
        ipc_socket_path: Option<String>,
    }
}

impl_from_ocaml_record! {
    OCamlForkingTestchainData => ForkingTestchainData {
        forking_block_hash: OCamlBlockHash,
        test_chain_id: OCamlChainId,
    }
}

impl_from_ocaml_record! {
    OCamlCycleRollsOwnerSnapshot => CycleRollsOwnerSnapshot {
        cycle: OCamlInt,
        seed_bytes: OCamlBytes,
        rolls_data: OCamlList<(OCamlBytes, OCamlList<OCamlInt>)>,
        last_roll: OCamlInt32,
    }
}

impl_from_ocaml_record! {
    OCamlApplyBlockResponse => ApplyBlockResponse {
        validation_result_message: OCamlBytes,
        context_hash: OCamlContextHash,
        protocol_hash: OCamlProtocolHash,
        next_protocol_hash: OCamlProtocolHash,
        block_header_proto_json: OCamlBytes,
        block_header_proto_metadata_bytes: OCamlBytes,
        operations_proto_metadata_bytes: OCamlList<OCamlList<OCamlBytes>>,
        max_operations_ttl: OCamlInt,
        last_allowed_fork_level: OCamlInt32,
        forking_testchain: bool,
        forking_testchain_data: Option<OCamlForkingTestchainData>,
        block_metadata_hash: Option<OCamlBlockMetadataHash>,
        ops_metadata_hashes: Option<OCamlList<OCamlList<OCamlOperationMetadataHash>>>,
        ops_metadata_hash: Option<OCamlOperationMetadataListListHash>,
        cycle_rolls_owner_snapshots: OCamlList<OCamlCycleRollsOwnerSnapshot>,
        new_protocol_constants_json: Option<String>,
        new_cycle_eras_json: Option<String>,
        commit_time: OCamlFloat,
    }
}

impl_from_ocaml_record! {
    OCamlBeginApplicationResponse => BeginApplicationResponse {
        result: String,
    }
}

impl_from_ocaml_record! {
    OCamlPrevalidatorWrapper => PrevalidatorWrapper {
        chain_id: OCamlChainId,
        protocol: OCamlProtocolHash,
        context_fitness: Option<OCamlList<OCamlBytes>>
    }
}

impl_from_ocaml_record! {
    OCamlApplied => Applied {
        hash: OCamlOperationHash,
        protocol_data_json: OCamlBytes,
    }
}

impl_from_ocaml_record! {
    OCamlOperationProtocolDataJsonWithErrorListJson => OperationProtocolDataJsonWithErrorListJson {
        protocol_data_json: OCamlBytes,
        error_json: OCamlBytes,
    }
}

impl_from_ocaml_record! {
    OCamlErrored => Errored {
        hash: OCamlOperationHash,
        is_endorsement: Option<bool>,
        protocol_data_json_with_error_json: OCamlOperationProtocolDataJsonWithErrorListJson,
    }
}

impl_from_ocaml_record! {
    OCamlValidateOperationResult => ValidateOperationResult {
        applied: OCamlList<OCamlApplied>,
        refused: OCamlList<OCamlErrored>,
        branch_refused: OCamlList<OCamlErrored>,
        branch_delayed: OCamlList<OCamlErrored>,
    }
}

impl_from_ocaml_record! {
    OCamlValidateOperationResponse => ValidateOperationResponse {
        prevalidator: OCamlPrevalidatorWrapper,
        result: OCamlValidateOperationResult,
    }
}

impl_from_ocaml_record! {
    OCamlHelpersPreapplyResponse => HelpersPreapplyResponse {
        body: OCamlBytes,
    }
}

impl_from_ocaml_record! {
    OCamlTezosErrorTrace => TezosErrorTrace {
        head_error_id: String,
        trace_json: String,
    }
}

impl_from_ocaml_variant! {
    OCamlProtocolRpcResponse => ProtocolRpcResponse {
        ProtocolRpcResponse::RPCConflict(s: Option<OCamlBytes>),
        ProtocolRpcResponse::RPCCreated(s: Option<OCamlBytes>),
        ProtocolRpcResponse::RPCError(s: Option<OCamlBytes>),
        ProtocolRpcResponse::RPCForbidden(s: Option<OCamlBytes>),
        ProtocolRpcResponse::RPCGone(s: Option<OCamlBytes>),
        ProtocolRpcResponse::RPCNoContent,
        ProtocolRpcResponse::RPCNotFound(s: Option<OCamlBytes>),
        ProtocolRpcResponse::RPCOk(s: OCamlBytes),
        ProtocolRpcResponse::RPCUnauthorized,
    }
}

impl_from_ocaml_variant! {
    OCamlProtocolRpcError => ProtocolRpcError {
        ProtocolRpcError::RPCErrorCannotParseBody(s: OCamlBytes),
        ProtocolRpcError::RPCErrorCannotParsePath(p: OCamlList<OCamlBytes>, d: OCamlRpcArgDesc, s: OCamlBytes),
        ProtocolRpcError::RPCErrorCannotParseQuery(s: OCamlBytes),
        ProtocolRpcError::RPCErrorInvalidMethodString(s: OCamlBytes),
        ProtocolRpcError::RPCErrorMethodNotAllowed(m: OCamlList<OCamlRpcMethod>),
        ProtocolRpcError::RPCErrorServiceNotFound,
    }
}

impl_from_ocaml_record! {
    OCamlRpcArgDesc => RpcArgDesc {
        name: OCamlBytes,
        descr: Option<OCamlBytes>,
    }
}

impl_from_ocaml_variant! {
    OCamlRpcMethod => RpcMethod {
        RpcMethod::DELETE,
        RpcMethod::GET,
        RpcMethod::PATCH,
        RpcMethod::POST,
        RpcMethod::PUT,
    }
}

// TODO: remove this after TE-207 has been solved
impl_from_ocaml_variant! {
    FfiPath => FfiPath {
        Left(path: FfiPath, right: OCamlHash) => {
            let path: FfiPath = path;
            FfiPath::Left(
                Box::new(
                    FfiPathLeft {
                        path,
                        right,
                    }))
        },
        Right(left: OCamlHash, path: FfiPath) => {
            let path: FfiPath = path;
            FfiPath::Right(
                Box::new(
                    FfiPathRight {
                        left,
                        path,
                    }))
        },
        Op => FfiPath::Op,
    }
}
