// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use znfe::{
    ocaml, OCaml, OCamlAllocResult, OCamlAllocToken, OCamlBytes, OCamlInt, OCamlInt32, OCamlInt64,
    OCamlList, ToOCaml,
};

use crypto::hash::{BlockHash, ContextHash, Hash, OperationListListHash, ProtocolHash};
use tezos_messages::p2p::encoding::prelude::{BlockHeader, Operation};

use crate::ffi::{
    ApplyBlockRequest, ApplyBlockResponse, BeginConstructionRequest, FfiRpcService,
    ForkingTestchainData, JsonRpcRequest, PrevalidatorWrapper, ProtocolJsonRpcRequest,
    ValidateOperationRequest,
};

use super::{
    FfiBlockHeader, FfiBlockHeaderShellHeader, FfiOperation, FfiOperationShellHeader,
    OCamlBlockHash, OCamlContextHash, OCamlOperationHash, OCamlOperationListListHash,
    OCamlProtocolHash,
};

// Headers
struct BlockHeaderShellHeader {}

struct OperationShellHeader {}

ocaml! {
    // Requests

    alloc fn alloc_apply_block_request(
        chain_id: OCamlBytes,
        block_header: BlockHeader,
        pred_header: BlockHeader,
        max_operations_ttl: OCamlInt,
        operations: OCamlList<OCamlList<Operation>>,
    ) -> ApplyBlockRequest;

    alloc fn alloc_apply_block_response(
        validation_result_message: OCamlBytes,
        context_hash: OCamlContextHash,
        block_header_proto_json: OCamlBytes,
        block_header_proto_metadata_json: OCamlBytes,
        operations_proto_metadata_json: OCamlBytes,
        max_operations_ttl: OCamlInt,
        last_allowed_fork_level: OCamlInt32,
        forking_testchain: bool,
        forking_testchain_data: Option<ForkingTestchainData>,
    ) -> ApplyBlockResponse;

    alloc fn alloc_forking_testchain_data(
        forking_block_hash: OCamlBlockHash,
        test_chain_id: OCamlBytes,
    ) -> ForkingTestchainData;

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
        proto_level: OCamlInt,
        predecessor: OCamlBlockHash,
        timestamp: OCamlInt64,
        validation_passes: OCamlInt,
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
        context_fitness: Option<OCamlList<OCamlBytes>>,
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
        let operations: Vec<Vec<FfiOperation>> = self
            .operations
            .iter()
            .map(|ops| ops.iter().map(|op| FfiOperation(op.clone())).collect())
            .collect();
        unsafe {
            alloc_apply_block_request(
                token,
                &self.chain_id,
                &FfiBlockHeader(self.block_header.clone()),
                &FfiBlockHeader(self.pred_header.clone()),
                &self.max_operations_ttl,
                &operations,
            )
        }
    }
}

unsafe impl ToOCaml<ApplyBlockResponse> for ApplyBlockResponse {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<ApplyBlockResponse> {
        unsafe {
            alloc_apply_block_response(
                token,
                &self.validation_result_message,
                &self.context_hash,
                &self.block_header_proto_json,
                &self.block_header_proto_metadata_json,
                &self.operations_proto_metadata_json,
                &self.max_operations_ttl,
                &self.last_allowed_fork_level,
                &self.forking_testchain,
                &self.forking_testchain_data,
            )
        }
    }
}

unsafe impl ToOCaml<ForkingTestchainData> for ForkingTestchainData {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<ForkingTestchainData> {
        unsafe {
            alloc_forking_testchain_data(token, &self.forking_block_hash, &self.test_chain_id)
        }
    }
}

unsafe impl ToOCaml<BeginConstructionRequest> for BeginConstructionRequest {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<BeginConstructionRequest> {
        unsafe {
            alloc_begin_construction_request(
                token,
                &self.chain_id,
                &FfiBlockHeader(self.predecessor.clone()),
                &self.protocol_data,
            )
        }
    }
}

macro_rules! to_ocaml_hash {
    ($ocaml_name:ident, $rust_name:ident, $allocate:ident) => {
        unsafe impl ToOCaml<$ocaml_name> for $rust_name {
            fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<$ocaml_name> {
                unsafe { $allocate(token, self) }
            }
        }
    };
}

to_ocaml_hash!(
    OCamlOperationListListHash,
    OperationListListHash,
    alloc_operation_list_list_hash
);
to_ocaml_hash!(OCamlOperationHash, Hash, alloc_operation_hash);
to_ocaml_hash!(OCamlBlockHash, BlockHash, alloc_block_hash);
to_ocaml_hash!(OCamlContextHash, ContextHash, alloc_context_hash);
to_ocaml_hash!(OCamlProtocolHash, ProtocolHash, alloc_protocol_hash);

unsafe impl ToOCaml<PrevalidatorWrapper> for PrevalidatorWrapper {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<PrevalidatorWrapper> {
        unsafe { alloc_prevalidator_wrapper(token, &self.chain_id, &self.protocol, &self.context_fitness) }
    }
}

unsafe impl ToOCaml<ValidateOperationRequest> for ValidateOperationRequest {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<ValidateOperationRequest> {
        unsafe {
            alloc_validate_operation_request(
                token,
                &self.prevalidator,
                &FfiOperation(self.operation.clone()),
            )
        }
    }
}

unsafe impl ToOCaml<JsonRpcRequest> for JsonRpcRequest {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<JsonRpcRequest> {
        unsafe { alloc_json_rpc_request(token, &self.body, &self.context_path) }
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
            FfiRpcService::HelpersForgeOperations => OCaml::of_int(5),
            FfiRpcService::ContextContract => OCaml::of_int(6),
        };
        unsafe { OCamlAllocResult::of(ocaml_int.raw()) }
    }
}

unsafe impl ToOCaml<ProtocolJsonRpcRequest> for ProtocolJsonRpcRequest {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<ProtocolJsonRpcRequest> {
        unsafe {
            alloc_protocol_json_rpc_request(
                token,
                &FfiBlockHeader(self.block_header.clone()),
                &self.chain_id,
                &self.chain_arg,
                &self.request,
                &self.ffi_service,
            )
        }
    }
}

unsafe impl<'a> ToOCaml<BlockHeaderShellHeader> for FfiBlockHeaderShellHeader<'a> {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<BlockHeaderShellHeader> {
        let ref block_header = (self.0).0;
        unsafe {
            alloc_block_header_shell_header(
                token,
                &block_header.level(),
                &(block_header.proto() as i32),
                block_header.predecessor(),
                &block_header.timestamp(),
                &(block_header.validation_pass() as i32),
                block_header.operations_hash(),
                block_header.fitness(),
                block_header.context(),
            )
        }
    }
}

unsafe impl ToOCaml<BlockHeader> for FfiBlockHeader {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<BlockHeader> {
        let ref block_header = self.0;
        let shell_header = FfiBlockHeaderShellHeader(self);
        unsafe { alloc_block_header(token, &shell_header, block_header.protocol_data()) }
    }
}

unsafe impl<'a> ToOCaml<OperationShellHeader> for FfiOperationShellHeader<'a> {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<OperationShellHeader> {
        unsafe { alloc_operation_shell_header(token, self.branch) }
    }
}

unsafe impl ToOCaml<Operation> for FfiOperation {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<Operation> {
        let ref operation = self.0;
        let shell = FfiOperationShellHeader {
            branch: operation.branch(),
        };
        unsafe { alloc_operation(token, &shell, operation.data()) }
    }
}
