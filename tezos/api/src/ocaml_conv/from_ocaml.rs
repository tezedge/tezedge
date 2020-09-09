// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use super::{
    FfiPath, OCamlBlockHash, OCamlContextHash, OCamlHash, OCamlOperationHash, OCamlProtocolHash,
};
use crate::ffi::{
    Applied, ApplyBlockResponse, Errored, ForkingTestchainData, JsonRpcResponse,
    OperationProtocolDataJsonWithErrorListJson, PrevalidatorWrapper, ValidateOperationResponse,
    ValidateOperationResult,
};
use crypto::hash::{BlockHash, ContextHash, Hash, OperationHash, ProtocolHash};
use tezos_messages::p2p::encoding::operations_for_blocks::{Path, PathLeft, PathRight};
use znfe::{FromOCaml, OCamlInt, IntoRust, OCaml, OCamlBytes, OCamlInt32, OCamlList};

macro_rules! from_ocaml_hash {
    ($ocaml_name:ident, $rust_name:ident) => {
        unsafe impl FromOCaml<$ocaml_name> for $rust_name {
            fn from_ocaml(v: OCaml<$ocaml_name>) -> Self {
                unsafe { v.field::<OCamlBytes>(0).into_rust() }
            }
        }
    }
}

from_ocaml_hash!(OCamlHash, Hash);
from_ocaml_hash!(OCamlOperationHash, OperationHash);
from_ocaml_hash!(OCamlBlockHash, BlockHash);
from_ocaml_hash!(OCamlContextHash, ContextHash);
from_ocaml_hash!(OCamlProtocolHash, ProtocolHash);

// Helper for easier mapping from OCaml to Rust records
macro_rules! unpack_ocaml_block {
    ($var:ident => $cons:ident {
        $($field:ident : $ocaml_typ:ty),+ $(,)?
    }) => {
        let record = $var;
        unsafe {
            let mut current = 0;

            $(
                current += 1;
                let $field = record.field::<$ocaml_typ>(current - 1).into_rust();
            )+

            $cons {
                $($field),+
            }
        }
    };
}

unsafe impl FromOCaml<ForkingTestchainData> for ForkingTestchainData {
    fn from_ocaml(v: OCaml<ForkingTestchainData>) -> Self {
        unpack_ocaml_block! { v =>
            ForkingTestchainData {
                forking_block_hash: OCamlBlockHash,
                test_chain_id: OCamlBytes,
            }
        }
    }
}

unsafe impl FromOCaml<ApplyBlockResponse> for ApplyBlockResponse {
    fn from_ocaml(v: OCaml<ApplyBlockResponse>) -> Self {
        unpack_ocaml_block! { v =>
            ApplyBlockResponse {
                validation_result_message: String,
                context_hash: OCamlContextHash,
                block_header_proto_json: String,
                block_header_proto_metadata_json: String,
                operations_proto_metadata_json: String,
                max_operations_ttl: OCamlInt,
                last_allowed_fork_level: OCamlInt32,
                forking_testchain: bool,
                forking_testchain_data: Option<ForkingTestchainData>,
            }
        }
    }
}

unsafe impl FromOCaml<PrevalidatorWrapper> for PrevalidatorWrapper {
    fn from_ocaml(v: OCaml<PrevalidatorWrapper>) -> Self {
        unpack_ocaml_block! { v =>
            PrevalidatorWrapper {
                chain_id: OCamlBytes,
                protocol: OCamlProtocolHash,
            }
        }
    }
}

unsafe impl FromOCaml<Applied> for Applied {
    fn from_ocaml(v: OCaml<Applied>) -> Self {
        unpack_ocaml_block! { v =>
            Applied {
                hash: OCamlOperationHash,
                protocol_data_json: OCamlBytes,
            }
        }
    }
}

unsafe impl FromOCaml<OperationProtocolDataJsonWithErrorListJson>
    for OperationProtocolDataJsonWithErrorListJson
{
    fn from_ocaml(v: OCaml<OperationProtocolDataJsonWithErrorListJson>) -> Self {
        unpack_ocaml_block! { v =>
            OperationProtocolDataJsonWithErrorListJson {
                protocol_data_json: OCamlBytes,
                error_json: OCamlBytes,
            }
        }
    }
}

unsafe impl FromOCaml<Errored> for Errored {
    fn from_ocaml(v: OCaml<Errored>) -> Self {
        unpack_ocaml_block! { v =>
            Errored {
                hash: OCamlOperationHash,
                is_endorsement: Option<bool>,
                protocol_data_json_with_error_json: OperationProtocolDataJsonWithErrorListJson,
            }
        }
    }
}

unsafe impl FromOCaml<ValidateOperationResult> for ValidateOperationResult {
    fn from_ocaml(v: OCaml<ValidateOperationResult>) -> Self {
        unpack_ocaml_block! { v =>
            ValidateOperationResult {
                applied: OCamlList<Applied>,
                refused: OCamlList<Errored>,
                branch_refused: OCamlList<Errored>,
                branch_delayed: OCamlList<Errored>,
            }
        }
    }
}

unsafe impl FromOCaml<ValidateOperationResponse> for ValidateOperationResponse {
    fn from_ocaml(v: OCaml<ValidateOperationResponse>) -> Self {
        unpack_ocaml_block! { v =>
            ValidateOperationResponse {
                prevalidator: PrevalidatorWrapper,
                result: ValidateOperationResult,
            }
        }
    }
}

unsafe impl FromOCaml<JsonRpcResponse> for JsonRpcResponse {
    fn from_ocaml(v: OCaml<JsonRpcResponse>) -> Self {
        unpack_ocaml_block! { v =>
            JsonRpcResponse {
                body: OCamlBytes,
            }
        }
    }
}

// TODO: remove this after TE-207 has been solved
unsafe impl FromOCaml<Path> for FfiPath {
    fn from_ocaml(v: OCaml<Path>) -> Self {
        if v.is_long() {
            FfiPath(Path::Op)
        } else {
            match v.tag_value() {
                0 => {
                    let path: FfiPath = unsafe { v.field::<Path>(0).into_rust() };
                    let right = unsafe { v.field::<OCamlHash>(1).into_rust() };

                    FfiPath(Path::Left(Box::new(PathLeft::new(
                        path.0,
                        right,
                        Default::default(), // TODO: what is body?
                    ))))
                }
                1 => {
                    let left = unsafe { v.field::<OCamlHash>(0).into_rust() };
                    let path: FfiPath = unsafe { v.field::<Path>(1).into_rust() };

                    FfiPath(Path::Right(Box::new(PathRight::new(
                        left,
                        path.0,
                        Default::default(), // TODO: what is body?
                    ))))
                }
                tag => panic!("Invalid tag value for OCaml<Path>: {}", tag),
            }
        }
    }
}
