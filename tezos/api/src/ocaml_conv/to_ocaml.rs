// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use super::{
    FfiBlockHeader, FfiBlockHeaderShellHeader, FfiOperation, FfiOperationShellHeader,
    OCamlBlockHash, OCamlContextHash, OCamlHash, OCamlOperationHash, OCamlOperationListListHash,
    OCamlProtocolHash, TaggedHash,
};
use crate::ffi::{
    ApplyBlockRequest, ApplyBlockResponse, BeginConstructionRequest, FfiRpcService,
    ForkingTestchainData, JsonRpcRequest, PrevalidatorWrapper, ProtocolJsonRpcRequest,
    ValidateOperationRequest,
};
use crypto::hash::{BlockHash, ContextHash, Hash, OperationListListHash, ProtocolHash};
use tezos_messages::p2p::encoding::prelude::{BlockHeader, Operation};
use znfe::{
    impl_to_ocaml_record, ocaml, ocaml_alloc_record, OCaml, OCamlAllocResult, OCamlAllocToken,
    OCamlBytes, OCamlInt, OCamlInt32, OCamlInt64, OCamlList, ToOCaml,
};

// OCaml type tags

struct BlockHeaderShellHeader {}
struct OperationShellHeader {}

ocaml! {
    alloc fn alloc_ocaml_hash(hash: OCamlBytes) -> OCamlHash;
    alloc fn alloc_operation_list_list_hash(hash: OCamlBytes) -> OCamlOperationListListHash;
    alloc fn alloc_operation_hash(hash: OCamlBytes) -> OCamlOperationHash;
    alloc fn alloc_block_hash(hash: OCamlBytes) -> OCamlBlockHash;
    alloc fn alloc_context_hash(hash: OCamlBytes) -> OCamlContextHash;
    alloc fn alloc_protocol_hash(hash: OCamlBytes) -> OCamlProtocolHash;
}

// Hashes

impl<'a> From<&'a Hash> for TaggedHash<'a> {
    fn from(bytes: &'a Hash) -> Self {
        Self(bytes)
    }
}

unsafe impl<'a> ToOCaml<OCamlHash> for TaggedHash<'a> {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<OCamlHash> {
        unsafe { alloc_ocaml_hash(token, self.0) }
    }
}

macro_rules! to_ocaml_hash {
    ($ocaml_name:ty, $rust_name:ty, $allocate:ident) => {
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

// Other

impl<'a> From<&'a BlockHeader> for FfiBlockHeaderShellHeader<'a> {
    fn from(block_header: &'a BlockHeader) -> Self {
        Self {
            level: block_header.level(),
            proto_level: block_header.proto() as i32,
            predecessor: block_header.predecessor().into(),
            timestamp: block_header.timestamp(),
            validation_passes: block_header.validation_pass() as i32,
            operations_hash: block_header.operations_hash().into(),
            fitness: block_header.fitness(),
            context: block_header.context().into(),
        }
    }
}

impl<'a> From<&'a BlockHeader> for FfiBlockHeader<'a> {
    fn from(block_header: &'a BlockHeader) -> Self {
        let shell = FfiBlockHeaderShellHeader::from(block_header);
        Self {
            shell,
            protocol_data: block_header.protocol_data(),
        }
    }
}

impl<'a> From<&'a Operation> for FfiOperationShellHeader<'a> {
    fn from(operation: &'a Operation) -> Self {
        Self {
            branch: TaggedHash(operation.branch()),
        }
    }
}

impl<'a> From<&'a Operation> for FfiOperation<'a> {
    fn from(operation: &'a Operation) -> Self {
        let shell = FfiOperationShellHeader::from(operation);
        Self {
            shell,
            data: operation.data(),
        }
    }
}

impl_to_ocaml_record! {
    ApplyBlockRequest {
        chain_id: OCamlBytes,
        block_header: BlockHeader => FfiBlockHeader::from(block_header),
        pred_header: BlockHeader => FfiBlockHeader::from(pred_header),
        max_operations_ttl: OCamlInt,
        operations: OCamlList<OCamlList<Operation>> => {
            operations.iter()
                      .map(|ops| ops.iter().map(|op| FfiOperation::from(op)).collect())
                      .collect::<Vec<Vec<FfiOperation>>>()
        },
    }
}

impl_to_ocaml_record! {
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

impl_to_ocaml_record! {
    ForkingTestchainData {
        forking_block_hash: OCamlBlockHash,
        test_chain_id: OCamlBytes,
    }
}

impl_to_ocaml_record! {
    BeginConstructionRequest {
        chain_id: OCamlBytes,
        predecessor: BlockHeader => FfiBlockHeader::from(predecessor),
        protocol_data: Option<OCamlBytes>,
    }
}

impl_to_ocaml_record! {
    PrevalidatorWrapper {
        chain_id: OCamlBytes,
        protocol: OCamlProtocolHash,
    }
}

impl_to_ocaml_record! {
    ValidateOperationRequest {
        prevalidator: PrevalidatorWrapper,
        operation: Operation => FfiOperation::from(operation),
    }
}

impl_to_ocaml_record! {
    JsonRpcRequest {
        body: OCamlBytes,
        context_path: OCamlBytes,
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

impl_to_ocaml_record! {
    ProtocolJsonRpcRequest {
        block_header: BlockHeader => FfiBlockHeader::from(block_header),
        chain_id: OCamlBytes,
        chain_arg: OCamlBytes,
        request: JsonRpcRequest,
        ffi_service: FfiRpcService,
    }
}

unsafe impl<'a> ToOCaml<BlockHeaderShellHeader> for FfiBlockHeaderShellHeader<'a> {
    fn to_ocaml(&self, _token: OCamlAllocToken) -> OCamlAllocResult<BlockHeaderShellHeader> {
        ocaml_alloc_record! {
            self => BlockHeaderShellHeader {
                level: OCamlInt32,
                proto_level: OCamlInt,
                predecessor: OCamlHash,
                timestamp: OCamlInt64,
                validation_passes: OCamlInt,
                operations_hash: OCamlHash,
                fitness: OCamlList<OCamlBytes>,
                context: OCamlHash,
            }
        }
    }
}

unsafe impl<'a> ToOCaml<BlockHeader> for FfiBlockHeader<'a> {
    fn to_ocaml(&self, _token: OCamlAllocToken) -> OCamlAllocResult<BlockHeader> {
        ocaml_alloc_record! {
            self => BlockHeader {
                shell: BlockHeaderShellHeader,
                protocol_data: OCamlBytes,
            }
        }
    }
}

unsafe impl<'a> ToOCaml<OperationShellHeader> for FfiOperationShellHeader<'a> {
    fn to_ocaml(&self, _token: OCamlAllocToken) -> OCamlAllocResult<OperationShellHeader> {
        ocaml_alloc_record! {
            self => OperationShellHeader {
                branch: OCamlHash,
            }
        }
    }
}

unsafe impl<'a> ToOCaml<Operation> for FfiOperation<'a> {
    fn to_ocaml(&self, _token: OCamlAllocToken) -> OCamlAllocResult<Operation> {
        ocaml_alloc_record! {
            self => Operation {
                shell: OperationShellHeader,
                data: OCamlBytes,
            }
        }
    }
}
