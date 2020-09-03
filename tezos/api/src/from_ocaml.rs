// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::ffi::{
    Applied, ApplyBlockResponse, Errored, ForkingTestchainData, JsonRpcResponse,
    OperationProtocolDataJsonWithErrorListJson, PrevalidatorWrapper, ValidateOperationResponse,
    ValidateOperationResult,
};
use crypto::hash::{BlockHash, OperationHash, ContextHash, ProtocolHash};
use znfe::{FromOCaml, Intnat, IntoRust, OCaml, OCamlBytes, OCamlInt32, OCamlList};

struct OCamlOperationHash {}
struct OCamlBlockHash {}
struct OCamlContextHash {}
struct OCamlProtocolHash {}

unsafe impl FromOCaml<OCamlOperationHash> for OperationHash {
    fn from_ocaml(v: OCaml<OCamlOperationHash>) -> Self {
        unsafe { v.field::<OCamlBytes>(0).into_rust() }
    }
}

unsafe impl FromOCaml<OCamlBlockHash> for BlockHash {
    fn from_ocaml(v: OCaml<OCamlBlockHash>) -> Self {
        unsafe { v.field::<OCamlBytes>(0).into_rust() }
    }
}

unsafe impl FromOCaml<OCamlContextHash> for ContextHash {
    fn from_ocaml(v: OCaml<OCamlContextHash>) -> Self {
        unsafe { v.field::<OCamlBytes>(0).into_rust() }
    }
}

unsafe impl FromOCaml<OCamlProtocolHash> for ProtocolHash {
    fn from_ocaml(v: OCaml<OCamlProtocolHash>) -> Self {
        unsafe { v.field::<OCamlBytes>(0).into_rust() }
    }
}

unsafe impl FromOCaml<ForkingTestchainData> for ForkingTestchainData {
    fn from_ocaml(v: OCaml<ForkingTestchainData>) -> Self {
        unsafe {
            ForkingTestchainData {
                forking_block_hash: v.field::<OCamlBlockHash>(0).into_rust(),
                test_chain_id: v.field::<OCamlBytes>(1).into_rust(),
            }
        }
    }
}

unsafe impl FromOCaml<ApplyBlockResponse> for ApplyBlockResponse {
    fn from_ocaml(v: OCaml<ApplyBlockResponse>) -> Self {
        unsafe {
            ApplyBlockResponse {
                validation_result_message: v.field::<String>(0).into_rust(),
                context_hash: v.field::<OCamlContextHash>(1).into_rust(),
                block_header_proto_json: v.field::<String>(2).into_rust(),
                block_header_proto_metadata_json: v.field::<String>(3).into_rust(),
                operations_proto_metadata_json: v.field::<String>(4).into_rust(),
                max_operations_ttl: (v.field::<Intnat>(5).as_int() as i32),
                last_allowed_fork_level: v.field::<OCamlInt32>(6).into_rust(),
                forking_testchain: v.field::<bool>(7).into_rust(),
                forking_testchain_data: v.field::<Option<ForkingTestchainData>>(8).into_rust(),
            }
        }
    }
}

unsafe impl FromOCaml<PrevalidatorWrapper> for PrevalidatorWrapper {
    fn from_ocaml(v: OCaml<PrevalidatorWrapper>) -> Self {
        unsafe {
            PrevalidatorWrapper {
                chain_id: v.field::<OCamlBytes>(0).into_rust(),
                protocol: v.field::<OCamlProtocolHash>(1).into_rust(),
            }
        }
    }
}

unsafe impl FromOCaml<Applied> for Applied {
    fn from_ocaml(v: OCaml<Applied>) -> Self {
        unsafe {
            Applied {
                hash: v.field::<OCamlOperationHash>(0).into_rust(),
                protocol_data_json: v.field::<OCamlBytes>(1).into_rust(),
            }
        }
    }
}

unsafe impl FromOCaml<OperationProtocolDataJsonWithErrorListJson>
    for OperationProtocolDataJsonWithErrorListJson
{
    fn from_ocaml(v: OCaml<OperationProtocolDataJsonWithErrorListJson>) -> Self {
        unsafe {
            OperationProtocolDataJsonWithErrorListJson {
                protocol_data_json: v.field::<OCamlBytes>(0).into_rust(),
                error_json: v.field::<OCamlBytes>(1).into_rust(),
            }
        }
    }
}

unsafe impl FromOCaml<Errored> for Errored {
    fn from_ocaml(v: OCaml<Errored>) -> Self {
        unsafe {
            Errored {
                hash: v.field::<OCamlOperationHash>(0).into_rust(),
                is_endorsement: v.field::<Option<bool>>(1).into_rust(),
                protocol_data_json_with_error_json: v
                    .field::<OperationProtocolDataJsonWithErrorListJson>(2)
                    .into_rust(),
            }
        }
    }
}

unsafe impl FromOCaml<ValidateOperationResult> for ValidateOperationResult {
    fn from_ocaml(v: OCaml<ValidateOperationResult>) -> Self {
        unsafe {
            ValidateOperationResult {
                applied: v.field::<OCamlList<Applied>>(0).into_rust(),
                refused: v.field::<OCamlList<Errored>>(1).into_rust(),
                branch_refused: v.field::<OCamlList<Errored>>(2).into_rust(),
                branch_delayed: v.field::<OCamlList<Errored>>(3).into_rust(),
            }
        }
    }
}

unsafe impl FromOCaml<ValidateOperationResponse> for ValidateOperationResponse {
    fn from_ocaml(v: OCaml<ValidateOperationResponse>) -> Self {
        unsafe {
            ValidateOperationResponse {
                prevalidator: v.field::<PrevalidatorWrapper>(0).into_rust(),
                result: v.field::<ValidateOperationResult>(1).into_rust(),
            }
        }
    }
}

unsafe impl FromOCaml<JsonRpcResponse> for JsonRpcResponse {
    fn from_ocaml(v: OCaml<JsonRpcResponse>) -> Self {
        unsafe {
            JsonRpcResponse {
                body: v.field::<OCamlBytes>(0).into_rust(),
            }
        }
    }
}
