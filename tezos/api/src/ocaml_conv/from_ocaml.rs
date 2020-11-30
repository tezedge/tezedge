// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use super::{
    FfiPath, OCamlBlockHash, OCamlContextHash, OCamlHash, OCamlOperationHash, OCamlProtocolHash,
};
use crate::ffi::{
    Applied, ApplyBlockResponse, BeginApplicationResponse, Errored, ForkingTestchainData,
    HelpersPreapplyResponse, OperationProtocolDataJsonWithErrorListJson, PrevalidatorWrapper,
    ProtocolRpcError, ProtocolRpcResponse, RpcArgDesc, RpcMethod, ValidateOperationResponse,
    ValidateOperationResult,
};
use crypto::hash::{BlockHash, ContextHash, Hash, OperationHash, ProtocolHash};
use ocaml_interop::{
    impl_from_ocaml_record, impl_from_ocaml_variant, FromOCaml, OCaml, OCamlBytes, OCamlInt,
    OCamlInt32, OCamlList, ToRust,
};
use tezos_messages::p2p::encoding::operations_for_blocks::{Path, PathLeft, PathRight};

macro_rules! from_ocaml_hash {
    ($ocaml_name:ident, $rust_name:ident) => {
        unsafe impl FromOCaml<$ocaml_name> for $rust_name {
            fn from_ocaml(v: OCaml<$ocaml_name>) -> Self {
                unsafe { v.field::<OCamlBytes>(0).to_rust() }
            }
        }
    };
}

from_ocaml_hash!(OCamlHash, Hash);
from_ocaml_hash!(OCamlOperationHash, OperationHash);
from_ocaml_hash!(OCamlBlockHash, BlockHash);
from_ocaml_hash!(OCamlContextHash, ContextHash);
from_ocaml_hash!(OCamlProtocolHash, ProtocolHash);

impl_from_ocaml_record! {
    ForkingTestchainData {
        forking_block_hash: OCamlBlockHash,
        test_chain_id: OCamlBytes,
    }
}

impl_from_ocaml_record! {
    ApplyBlockResponse {
        validation_result_message: OCamlBytes,
        context_hash: OCamlContextHash,
        block_header_proto_json: OCamlBytes,
        block_header_proto_metadata_json: OCamlBytes,
        operations_proto_metadata_json: OCamlBytes,
        max_operations_ttl: OCamlInt,
        last_allowed_fork_level: OCamlInt32,
        forking_testchain: bool,
        forking_testchain_data: Option<ForkingTestchainData>,
    }
}

impl_from_ocaml_record! {
    BeginApplicationResponse {
        result: String,
    }
}

impl_from_ocaml_record! {
    PrevalidatorWrapper {
        chain_id: OCamlBytes,
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
    Path => FfiPath {
        Left(path: Path, right: OCamlHash) => {
            let path: FfiPath = path;
            FfiPath(
                Path::Left(
                    Box::new(
                        PathLeft::new(
                            path.0,
                            right,
                            Default::default()))))
        },
        Right(left: OCamlHash, path: Path) => {
            let path: FfiPath = path;
            FfiPath(
                Path::Right(
                    Box::new(
                        PathRight::new(
                            left,
                            path.0,
                            Default::default()))))
        },
        Op => FfiPath(Path::Op),
    }
}
