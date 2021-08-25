// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;

use super::{
    FfiPath, FfiPathLeft, FfiPathRight, OCamlBlockHash, OCamlBlockMetadataHash, OCamlChainId,
    OCamlContextHash, OCamlHash, OCamlOperationHash, OCamlOperationMetadataHash,
    OCamlOperationMetadataListListHash, OCamlProtocolHash,
};
use crate::ffi::{
    Applied, ApplyBlockResponse, BeginApplicationResponse, ContextKvStoreConfiguration,
    CycleRollsOwnerSnapshot, Errored, ForkingTestchainData, HelpersPreapplyResponse,
    OperationProtocolDataJsonWithErrorListJson, PrevalidatorWrapper, ProtocolRpcError,
    ProtocolRpcResponse, RpcArgDesc, RpcMethod, TezosContextTezEdgeStorageConfiguration,
    TezosErrorTrace, ValidateOperationResponse, ValidateOperationResult,
};
use crypto::hash::{
    BlockHash, BlockMetadataHash, ChainId, ContextHash, Hash, OperationHash, OperationMetadataHash,
    OperationMetadataListListHash, ProtocolHash,
};
use ocaml_interop::{
    impl_from_ocaml_record, impl_from_ocaml_variant, FromOCaml, OCaml, OCamlBytes, OCamlFloat,
    OCamlInt, OCamlInt32, OCamlList,
};

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
    ContextKvStoreConfiguration {
        ContextKvStoreConfiguration::ReadOnlyIpc,
        ContextKvStoreConfiguration::InMem,
    }
}

impl_from_ocaml_record! {
    TezosContextTezEdgeStorageConfiguration {
        backend: ContextKvStoreConfiguration,
        ipc_socket_path: Option<String>,
    }
}

impl_from_ocaml_record! {
    ForkingTestchainData {
        forking_block_hash: OCamlBlockHash,
        test_chain_id: OCamlChainId,
    }
}

impl_from_ocaml_record! {
    CycleRollsOwnerSnapshot {
        cycle: OCamlInt,
        seed_bytes: OCamlBytes,
        rolls_data: OCamlList<(OCamlBytes, OCamlList<OCamlInt>)>,
        last_roll: OCamlInt32,
    }
}

impl_from_ocaml_record! {
    ApplyBlockResponse {
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
        forking_testchain_data: Option<ForkingTestchainData>,
        block_metadata_hash: Option<OCamlBlockMetadataHash>,
        ops_metadata_hashes: Option<OCamlList<OCamlList<OCamlOperationMetadataHash>>>,
        ops_metadata_hash: Option<OCamlOperationMetadataListListHash>,
        cycle_rolls_owner_snapshots: OCamlList<CycleRollsOwnerSnapshot>,
        new_protocol_constants_json: Option<String>,
        new_cycle_eras_json: Option<String>,
        commit_time: OCamlFloat,
    }
}

impl_from_ocaml_record! {
    BeginApplicationResponse {
        result: String,
    }
}

impl_from_ocaml_record! {
    PrevalidatorWrapper {
        chain_id: OCamlChainId,
        protocol: OCamlProtocolHash,
        context_fitness: Option<OCamlList<OCamlBytes>>
    }
}

impl_from_ocaml_record! {
    Applied {
        hash: OCamlOperationHash,
        protocol_data_json: OCamlBytes,
    }
}

impl_from_ocaml_record! {
    OperationProtocolDataJsonWithErrorListJson {
        protocol_data_json: OCamlBytes,
        error_json: OCamlBytes,
    }
}

impl_from_ocaml_record! {
    Errored {
        hash: OCamlOperationHash,
        is_endorsement: Option<bool>,
        protocol_data_json_with_error_json: OperationProtocolDataJsonWithErrorListJson,
    }
}

impl_from_ocaml_record! {
    ValidateOperationResult {
        applied: OCamlList<Applied>,
        refused: OCamlList<Errored>,
        branch_refused: OCamlList<Errored>,
        branch_delayed: OCamlList<Errored>,
    }
}

impl_from_ocaml_record! {
    ValidateOperationResponse {
        prevalidator: PrevalidatorWrapper,
        result: ValidateOperationResult,
    }
}

impl_from_ocaml_record! {
    HelpersPreapplyResponse {
        body: OCamlBytes,
    }
}

impl_from_ocaml_record! {
    TezosErrorTrace {
        head_error_id: String,
        trace_json: String,
    }
}

impl_from_ocaml_variant! {
    ProtocolRpcResponse {
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
    ProtocolRpcError {
        ProtocolRpcError::RPCErrorCannotParseBody(s: OCamlBytes),
        ProtocolRpcError::RPCErrorCannotParsePath(p: OCamlList<OCamlBytes>, d: RpcArgDesc, s: OCamlBytes),
        ProtocolRpcError::RPCErrorCannotParseQuery(s: OCamlBytes),
        ProtocolRpcError::RPCErrorInvalidMethodString(s: OCamlBytes),
        ProtocolRpcError::RPCErrorMethodNotAllowed(m: OCamlList<RpcMethod>),
        ProtocolRpcError::RPCErrorServiceNotFound,
    }
}

impl_from_ocaml_record! {
    RpcArgDesc {
        name: OCamlBytes,
        descr: Option<OCamlBytes>,
    }
}

impl_from_ocaml_variant! {
    RpcMethod {
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
