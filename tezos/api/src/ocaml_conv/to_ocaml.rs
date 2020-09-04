// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use super::{
    OCamlBlockHash, OCamlContextHash, OCamlOperationHash, OCamlOperationListListHash,
    OCamlProtocolHash,
};
use crate::ffi::{
    ApplyBlockRequest, BeginConstructionRequest, FfiRpcService, JsonRpcRequest,
    PrevalidatorWrapper, ProtocolJsonRpcRequest, ValidateOperationRequest,
};
use crypto::hash::{BlockHash, ContextHash, OperationListListHash, ProtocolHash, Hash};
use tezos_messages::p2p::encoding::prelude::{BlockHeader, Operation};
use znfe::{
    ocaml, ocaml_alloc, ocaml_frame, to_ocaml, Intnat, OCaml, OCamlAllocResult, OCamlAllocToken,
    OCamlBytes, OCamlInt32, OCamlInt64, OCamlList, ToOCaml,
};

// FFI Wrappers
#[repr(transparent)]
struct FfiBlockHeader(pub *const BlockHeader);
#[repr(transparent)]
struct FfiOperation(pub *const Operation);

// Headers
struct BlockHeaderShellHeader {}
struct OperationShellHeader {}

ocaml! {
    // Requests

    alloc fn alloc_apply_block_request(
        chain_id: OCamlBytes,
        block_header: BlockHeader,
        pred_header: BlockHeader,
        max_operations_ttl: Intnat,
        operations: OCamlList<OCamlList<Operation>>,
    ) -> ApplyBlockRequest;

    alloc fn alloc_begin_construction_request(
        chain_id: OCamlBytes,
        predecessor: BlockHeader,
        protocol_data: Option<OCamlBytes>,
    ) -> BeginConstructionRequest;

    alloc fn alloc_validate_operation_request(
        prevalidator: PrevalidatorWrapper,
        operation: Operation,
    ) -> ValidateOperationRequest;

    alloc fn alloc_json_rpc_request(
        body: OCamlBytes,
        context_path: OCamlBytes,
    ) -> JsonRpcRequest;

    alloc fn alloc_protocol_json_rpc_request(
        block_header: BlockHeader,
        chain_id: OCamlBytes,
        chain_arg: OCamlBytes,
        request: JsonRpcRequest,
        ffi_service: FfiRpcService,
    ) -> ProtocolJsonRpcRequest;

    // Headers

    alloc fn alloc_block_header_shell_header(
        level: OCamlInt32,
        proto_level: Intnat,
        predecessor: OCamlBlockHash,
        timestamp: OCamlInt64,
        validation_passes: Intnat,
        operations_hash: OCamlOperationListListHash,
        fitness: OCamlList<OCamlBytes>,
        context: OCamlContextHash,
    ) -> BlockHeaderShellHeader;

    alloc fn alloc_block_header(
        shell: BlockHeaderShellHeader,
        protocol_data: OCamlBytes,
    ) -> BlockHeader;

    alloc fn alloc_operation_shell_header(
        branch: OCamlBlockHash,
    ) -> OperationShellHeader;

    // Operation

    alloc fn alloc_operation(
        shell: OperationShellHeader,
        proto: OCamlBytes,
    ) -> Operation;

    // Wrappers

    alloc fn alloc_prevalidator_wrapper(
        chain_id: OCamlBytes,
        protocol: OCamlProtocolHash,
    ) -> PrevalidatorWrapper;

    // Hashes

    alloc fn alloc_operation_list_list_hash(hash: OCamlBytes) -> OCamlOperationListListHash;
    alloc fn alloc_operation_hash(hash: OCamlBytes) -> OCamlOperationHash;
    alloc fn alloc_block_hash(hash: OCamlBytes) -> OCamlBlockHash;
    alloc fn alloc_context_hash(hash: OCamlBytes) -> OCamlContextHash;
    alloc fn alloc_protocol_hash(hash: OCamlBytes) -> OCamlProtocolHash;
}

unsafe impl ToOCaml<ApplyBlockRequest> for ApplyBlockRequest {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<ApplyBlockRequest> {
        ocaml_frame!(gc, {
            let ref chain_id = to_ocaml!(gc, self.chain_id).keep(gc);
            let ref block_header = to_ocaml!(gc, FfiBlockHeader(&self.block_header)).keep(gc);
            let ref pred_header = to_ocaml!(gc, FfiBlockHeader(&self.pred_header)).keep(gc);
            let max_operations_ttl = OCaml::of_int(self.max_operations_ttl as i64);
            let operations: Vec<Vec<FfiOperation>> = self
                .operations
                .iter()
                .map(|ops| ops.iter().map(|op| FfiOperation(op)).collect())
                .collect();
            let operations = to_ocaml!(gc, operations);
            unsafe {
                alloc_apply_block_request(
                    token,
                    gc.get(chain_id),
                    gc.get(block_header),
                    gc.get(pred_header),
                    max_operations_ttl,
                    operations,
                )
            }
        })
    }
}

unsafe impl ToOCaml<BeginConstructionRequest> for BeginConstructionRequest {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<BeginConstructionRequest> {
        ocaml_frame!(gc, {
            let ref chain_id = to_ocaml!(gc, self.chain_id).keep(gc);
            let ref predecessor = to_ocaml!(gc, FfiBlockHeader(&self.predecessor)).keep(gc);
            let protocol_data = to_ocaml!(gc, self.protocol_data);
            unsafe {
                alloc_begin_construction_request(
                    token,
                    gc.get(chain_id),
                    gc.get(predecessor),
                    protocol_data,
                )
            }
        })
    }
}

macro_rules! to_ocaml_hash {
    ($ocaml_name:ident, $rust_name:ident, $allocate:ident) => {
        unsafe impl ToOCaml<$ocaml_name> for $rust_name {
            fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<$ocaml_name> {
                ocaml_frame!(gc, {
                    let hash = to_ocaml!(gc, self);
                    unsafe { $allocate(token, hash) }
                })
            }
        }
    };
}

to_ocaml_hash!(OCamlOperationListListHash, OperationListListHash, alloc_operation_list_list_hash);
to_ocaml_hash!(OCamlOperationHash, Hash, alloc_operation_hash);
to_ocaml_hash!(OCamlBlockHash, BlockHash, alloc_block_hash);
to_ocaml_hash!(OCamlContextHash, ContextHash, alloc_context_hash);
to_ocaml_hash!(OCamlProtocolHash, ProtocolHash, alloc_protocol_hash);

unsafe impl ToOCaml<PrevalidatorWrapper> for PrevalidatorWrapper {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<PrevalidatorWrapper> {
        ocaml_frame!(gc, {
            let ref chain_id = to_ocaml!(gc, self.chain_id).keep(gc);
            let protocol = to_ocaml!(gc, self.protocol);
            unsafe { alloc_prevalidator_wrapper(token, gc.get(chain_id), protocol) }
        })
    }
}

unsafe impl ToOCaml<ValidateOperationRequest> for ValidateOperationRequest {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<ValidateOperationRequest> {
        ocaml_frame!(gc, {
            let ref prevalidator = to_ocaml!(gc, self.prevalidator).keep(gc);
            let operation = to_ocaml!(gc, FfiOperation(&self.operation));
            unsafe { alloc_validate_operation_request(token, gc.get(prevalidator), operation) }
        })
    }
}

unsafe impl ToOCaml<JsonRpcRequest> for JsonRpcRequest {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<JsonRpcRequest> {
        ocaml_frame!(gc, {
            let ref body = to_ocaml!(gc, self.body).keep(gc);
            let context_path = to_ocaml!(gc, self.context_path);
            unsafe { alloc_json_rpc_request(token, gc.get(body), context_path) }
        })
    }
}

unsafe impl ToOCaml<FfiRpcService> for FfiRpcService {
    fn to_ocaml(&self, _token: OCamlAllocToken) -> OCamlAllocResult<FfiRpcService> {
        let ocaml_int = match self {
            FfiRpcService::HelpersRunOperation => OCaml::of_int(0),
            FfiRpcService::HelpersPreapplyOperations => OCaml::of_int(1),
            FfiRpcService::HelpersPreapplyBlock => OCaml::of_int(2),
            FfiRpcService::HelpersCurrentLevel => OCaml::of_int(3),
            FfiRpcService::DelegatesMinimalValidTime => OCaml::of_int(4),
        };
        unsafe { OCamlAllocResult::of(ocaml_int.raw()) }
    }
}

unsafe impl ToOCaml<ProtocolJsonRpcRequest> for ProtocolJsonRpcRequest {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<ProtocolJsonRpcRequest> {
        ocaml_frame!(gc, {
            let ref block_header = to_ocaml!(gc, FfiBlockHeader(&self.block_header)).keep(gc);
            let ref chain_id = to_ocaml!(gc, self.chain_id).keep(gc);
            let ref chain_arg = to_ocaml!(gc, self.chain_arg).keep(gc);
            let ref request = to_ocaml!(gc, self.request).keep(gc);
            let ffi_service = to_ocaml!(gc, self.ffi_service);
            unsafe {
                alloc_protocol_json_rpc_request(
                    token,
                    gc.get(block_header),
                    gc.get(chain_id),
                    gc.get(chain_arg),
                    gc.get(request),
                    ffi_service,
                )
            }
        })
    }
}

unsafe impl ToOCaml<BlockHeader> for FfiBlockHeader {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<BlockHeader> {
        ocaml_frame!(gc, {
            let block_header = unsafe { self.0.as_ref() }.unwrap();
            let ref level = to_ocaml!(gc, block_header.level()).keep(gc);
            let proto_level = OCaml::of_int(block_header.proto() as i64);
            let ref predecessor = to_ocaml!(gc, block_header.predecessor()).keep(gc);
            let ref timestamp = to_ocaml!(gc, block_header.timestamp()).keep(gc);
            let validation_passes = OCaml::of_int(block_header.validation_pass() as i64);
            let ref operations_hash = to_ocaml!(gc, block_header.operations_hash()).keep(gc);
            let ref fitness = to_ocaml!(gc, block_header.fitness()).keep(gc);
            let ref context = to_ocaml!(gc, block_header.context()).keep(gc);
            let ref protocol_data = to_ocaml!(gc, block_header.protocol_data()).keep(gc);
            let shell_header = unsafe {
                ocaml_alloc!(alloc_block_header_shell_header(
                    gc,
                    gc.get(level),
                    proto_level,
                    gc.get(predecessor),
                    gc.get(timestamp),
                    validation_passes,
                    gc.get(operations_hash),
                    gc.get(fitness),
                    gc.get(context),
                ))
            };
            unsafe { alloc_block_header(token, shell_header, gc.get(protocol_data)) }
        })
    }
}

unsafe impl ToOCaml<Operation> for FfiOperation {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<Operation> {
        ocaml_frame!(gc, {
            let operation = unsafe { self.0.as_ref() }.unwrap();
            let ref branch = to_ocaml!(gc, operation.branch()).keep(gc);
            let ref proto = to_ocaml!(gc, operation.data()).keep(gc);
            let shell = unsafe { ocaml_alloc!(alloc_operation_shell_header(gc, gc.get(branch))) };
            unsafe { alloc_operation(token, shell, gc.get(proto)) }
        })
    }
}
