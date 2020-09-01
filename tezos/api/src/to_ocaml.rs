// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::ffi::{
    ApplyBlockRequest, BeginConstructionRequest, FfiRpcService, JsonRpcRequest,
    PrevalidatorWrapper, ProtocolJsonRpcRequest, ValidateOperationRequest,
};
use tezos_messages::p2p::encoding::prelude::{BlockHeader, Operation};
use znfe::{
    ocaml, ocaml_frame, to_ocaml, Intnat, OCaml, OCamlAllocResult, OCamlAllocToken, OCamlBytes,
    OCamlList, ToOCaml,
};

ocaml! {
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

    alloc fn alloc_prevalidator_wrapper(
        chain_id: OCamlBytes,
        protocol: OCamlBytes,
    ) -> PrevalidatorWrapper;

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
}

unsafe impl ToOCaml<ApplyBlockRequest> for ApplyBlockRequest {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<ApplyBlockRequest> {
        ocaml_frame!(gc, {
            let ref chain_id = to_ocaml!(gc, self.chain_id).keep(gc);
            let ref block_header = to_ocaml!(gc, self.block_header).keep(gc);
            let ref pred_header = to_ocaml!(gc, self.pred_header).keep(gc);
            let max_operations_ttl = OCaml::of_int(self.max_operations_ttl as i64);
            let operations = to_ocaml!(gc, self.operations);
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
            let ref predecessor = to_ocaml!(gc, self.predecessor).keep(gc);
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
            let operation = to_ocaml!(gc, self.operation);
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
            FfiRpcService::LiveBlocks => OCaml::of_int(5),
        };
        unsafe { OCamlAllocResult::of(ocaml_int.raw()) }
    }
}

unsafe impl ToOCaml<ProtocolJsonRpcRequest> for ProtocolJsonRpcRequest {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<ProtocolJsonRpcRequest> {
        ocaml_frame!(gc, {
            let ref block_header = to_ocaml!(gc, self.block_header).keep(gc);
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
