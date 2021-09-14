// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{
    OCamlApplyBlockRequest, OCamlBeginApplicationRequest, OCamlBeginConstructionRequest,
    OCamlBlockHeader, OCamlBlockHeaderShellHeader, OCamlCycleRollsOwnerSnapshot,
    OCamlHelpersPreapplyBlockRequest, OCamlOperation, OCamlOperationShellHeader,
    OCamlProtocolRpcRequest, OCamlRpcRequest, OCamlTezosContextConfiguration,
    OCamlTezosContextIrminStorageConfiguration, OCamlTezosContextStorageConfiguration,
    OCamlValidateOperationRequest,
};

use super::{
    FfiBlockHeader, FfiBlockHeaderShellHeader, FfiOperation, FfiOperationShellHeader,
    OCamlApplyBlockResponse, OCamlBlockHash, OCamlBlockMetadataHash, OCamlChainId,
    OCamlContextHash, OCamlContextKvStoreConfiguration, OCamlForkingTestchainData, OCamlHash,
    OCamlOperationHash, OCamlOperationListListHash, OCamlOperationMetadataHash,
    OCamlOperationMetadataListListHash, OCamlPrevalidatorWrapper, OCamlProtocolHash,
    OCamlRpcMethod, OCamlTezosContextTezEdgeStorageConfiguration, TaggedHash,
};
use crypto::hash::{
    BlockHash, BlockMetadataHash, ChainId, ContextHash, Hash, OperationHash, OperationListListHash,
    OperationMetadataHash, OperationMetadataListListHash, ProtocolHash,
};
use ocaml_interop::{
    impl_to_ocaml_record, impl_to_ocaml_variant, ocaml_alloc_record, ocaml_alloc_variant, OCaml,
    OCamlBytes, OCamlFloat, OCamlInt, OCamlInt32, OCamlInt64, OCamlList, OCamlRuntime, ToOCaml,
};
use tezos_api::ffi::{
    ApplyBlockRequest, ApplyBlockResponse, BeginApplicationRequest, BeginConstructionRequest,
    CycleRollsOwnerSnapshot, ForkingTestchainData, HelpersPreapplyBlockRequest,
    PrevalidatorWrapper, ProtocolRpcRequest, RpcMethod, RpcRequest, ValidateOperationRequest,
};
use tezos_context_api::{
    ContextKvStoreConfiguration, TezosContextConfiguration, TezosContextIrminStorageConfiguration,
    TezosContextStorageConfiguration, TezosContextTezEdgeStorageConfiguration,
};
use tezos_messages::p2p::encoding::prelude::{BlockHeader, Operation};

// Hashes

impl<'a> From<&'a Hash> for TaggedHash<'a> {
    fn from(bytes: &'a Hash) -> Self {
        TaggedHash::Hash(bytes)
    }
}

unsafe impl<'a> ToOCaml<OCamlHash> for TaggedHash<'a> {
    fn to_ocaml<'gc>(&self, cr: &'gc mut OCamlRuntime) -> OCaml<'gc, OCamlHash> {
        ocaml_alloc_variant! {
            cr, self => {
                TaggedHash::Hash(hash: OCamlBytes)
            }
        }
    }
}

macro_rules! to_ocaml_hash {
    ($ocaml_name:ty, $rust_name:ty) => {
        unsafe impl ToOCaml<$ocaml_name> for $rust_name {
            fn to_ocaml<'gc>(&self, cr: &'gc mut OCamlRuntime) -> OCaml<'gc, $ocaml_name> {
                let tagged = TaggedHash::Hash(self.as_ref());
                ocaml_alloc_variant! {
                    cr, tagged => {
                        TaggedHash::Hash(hash: OCamlBytes)
                    }
                }
            }
        }
    };
}

to_ocaml_hash!(OCamlOperationListListHash, OperationListListHash);
to_ocaml_hash!(OCamlOperationHash, OperationHash);
to_ocaml_hash!(OCamlBlockHash, BlockHash);
to_ocaml_hash!(OCamlContextHash, ContextHash);
to_ocaml_hash!(OCamlProtocolHash, ProtocolHash);
to_ocaml_hash!(OCamlBlockMetadataHash, BlockMetadataHash);
to_ocaml_hash!(OCamlOperationMetadataHash, OperationMetadataHash);
to_ocaml_hash!(
    OCamlOperationMetadataListListHash,
    OperationMetadataListListHash
);

// Configuration

impl_to_ocaml_record! {
    TezosContextIrminStorageConfiguration => OCamlTezosContextIrminStorageConfiguration {
        data_dir: String,
    }
}

impl_to_ocaml_variant! {
    ContextKvStoreConfiguration => OCamlContextKvStoreConfiguration {
        ContextKvStoreConfiguration::ReadOnlyIpc,
        ContextKvStoreConfiguration::InMem,
    }
}

impl_to_ocaml_record! {
    TezosContextTezEdgeStorageConfiguration => OCamlTezosContextTezEdgeStorageConfiguration {
        backend: OCamlContextKvStoreConfiguration,
        ipc_socket_path: Option<String>,
    }
}

impl_to_ocaml_variant! {
    TezosContextStorageConfiguration => OCamlTezosContextStorageConfiguration {
        TezosContextStorageConfiguration::IrminOnly(cfg: OCamlTezosContextIrminStorageConfiguration),
        TezosContextStorageConfiguration::TezEdgeOnly(cfg: OCamlTezosContextTezEdgeStorageConfiguration),
        TezosContextStorageConfiguration::Both(
            irmin: OCamlTezosContextIrminStorageConfiguration,
            tezedge: OCamlTezosContextTezEdgeStorageConfiguration,
        )
    }
}

impl_to_ocaml_record! {
    TezosContextConfiguration => OCamlTezosContextConfiguration {
        storage: OCamlTezosContextStorageConfiguration,
        genesis: (String, String, String) =>
            (genesis.time.clone(), genesis.block.clone(), genesis.protocol.clone()),
        protocol_overrides: (OCamlList<(OCamlInt32, String)>, OCamlList<(String, String)>) =>
            (protocol_overrides.user_activated_upgrades.clone(),
             protocol_overrides.user_activated_protocol_overrides.clone()),
        commit_genesis: bool,
        enable_testchain: bool,
        readonly: bool,
        sandbox_json_patch_context: Option<(String, String)> =>
            sandbox_json_patch_context.clone().map(|pc| (pc.key, pc.json)),
        context_stats_db_path: Option<String> =>
            context_stats_db_path.clone().map(|pb| pb.to_string_lossy().into_owned()),
    }
}

// Other

// TODO: TE-367: review once ocaml-interop has been upgraded
unsafe impl ToOCaml<OCamlChainId> for ChainId {
    fn to_ocaml<'gc>(&self, cr: &'gc mut OCamlRuntime) -> OCaml<'gc, OCamlChainId> {
        let ocaml_bytes: OCaml<OCamlBytes> = self.0.to_ocaml(cr);
        unsafe { std::mem::transmute(ocaml_bytes) }
    }
}

impl<'a> From<&'a BlockHeader> for FfiBlockHeaderShellHeader<'a> {
    fn from(block_header: &'a BlockHeader) -> Self {
        let predecessor_hash = block_header.predecessor();
        let operations_hash = block_header.operations_hash();
        let context = block_header.context();
        Self {
            level: block_header.level(),
            proto_level: block_header.proto() as i32,
            predecessor: predecessor_hash,
            timestamp: block_header.timestamp(),
            validation_passes: block_header.validation_pass() as i32,
            operations_hash: operations_hash,
            fitness: block_header.fitness(),
            context: context,
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
            branch: operation.branch(),
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
    ApplyBlockRequest => OCamlApplyBlockRequest{
        chain_id: OCamlChainId,
        block_header: OCamlBlockHeader => FfiBlockHeader::from(block_header),
        pred_header: OCamlBlockHeader => FfiBlockHeader::from(pred_header),
        max_operations_ttl: OCamlInt,
        operations: OCamlList<OCamlList<OCamlOperation>> => {
            operations.iter()
                      .map(|ops| ops.iter().map(FfiOperation::from).collect())
                      .collect::<Vec<Vec<FfiOperation>>>()
        },
        predecessor_block_metadata_hash: Option<OCamlBlockMetadataHash>,
        predecessor_ops_metadata_hash: Option<OCamlOperationMetadataListListHash>,
    }
}

impl_to_ocaml_record! {
    CycleRollsOwnerSnapshot => OCamlCycleRollsOwnerSnapshot {
        cycle: OCamlInt,
        seed_bytes: OCamlBytes,
        rolls_data: OCamlList<(OCamlBytes, OCamlList<OCamlInt>)>,
        last_roll: OCamlInt32,
    }
}

impl_to_ocaml_record! {
    ApplyBlockResponse => OCamlApplyBlockResponse {
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

impl_to_ocaml_record! {
    ForkingTestchainData => OCamlForkingTestchainData {
        forking_block_hash: OCamlBlockHash,
        test_chain_id: OCamlChainId,
    }
}

impl_to_ocaml_record! {
    BeginApplicationRequest => OCamlBeginApplicationRequest {
        chain_id: OCamlChainId,
        pred_header: OCamlBlockHeader => FfiBlockHeader::from(pred_header),
        block_header: OCamlBlockHeader => FfiBlockHeader::from(block_header),
    }
}

impl_to_ocaml_record! {
    BeginConstructionRequest => OCamlBeginConstructionRequest {
        chain_id: OCamlChainId,
        predecessor: OCamlBlockHeader => FfiBlockHeader::from(predecessor),
        protocol_data: Option<OCamlBytes>,
    }
}

impl_to_ocaml_record! {
    PrevalidatorWrapper => OCamlPrevalidatorWrapper {
        chain_id: OCamlChainId,
        protocol: OCamlProtocolHash,
        context_fitness: Option<OCamlList<OCamlBytes>>
    }
}

impl_to_ocaml_record! {
    ValidateOperationRequest => OCamlValidateOperationRequest {
        prevalidator: OCamlPrevalidatorWrapper,
        operation: OCamlOperation => FfiOperation::from(operation),
    }
}

impl_to_ocaml_record! {
    RpcRequest => OCamlRpcRequest {
        body: OCamlBytes,
        context_path: OCamlBytes,
        meth: OCamlRpcMethod,
        content_type: Option<OCamlBytes>,
        accept: Option<OCamlBytes>,
    }
}

impl_to_ocaml_variant! {
    RpcMethod => OCamlRpcMethod {
        RpcMethod::DELETE,
        RpcMethod::GET,
        RpcMethod::PATCH,
        RpcMethod::POST,
        RpcMethod::PUT,
    }
}

impl_to_ocaml_record! {
    ProtocolRpcRequest => OCamlProtocolRpcRequest {
        block_header: OCamlBlockHeader => FfiBlockHeader::from(block_header),
        chain_id: OCamlChainId,
        chain_arg: OCamlBytes,
        request: OCamlRpcRequest,
    }
}

impl_to_ocaml_record! {
    HelpersPreapplyBlockRequest => OCamlHelpersPreapplyBlockRequest {
        protocol_rpc_request: OCamlProtocolRpcRequest,
        predecessor_block_metadata_hash: Option<OCamlBlockMetadataHash>,
        predecessor_ops_metadata_hash: Option<OCamlOperationMetadataListListHash>,
    }
}

unsafe impl<'a> ToOCaml<OCamlBlockHeaderShellHeader> for FfiBlockHeaderShellHeader<'a> {
    fn to_ocaml<'gc>(&self, cr: &'gc mut OCamlRuntime) -> OCaml<'gc, OCamlBlockHeaderShellHeader> {
        ocaml_alloc_record! {
            cr, self {
                level: OCamlInt32,
                proto_level: OCamlInt,
                predecessor: OCamlBlockHash,
                timestamp: OCamlInt64,
                validation_passes: OCamlInt,
                operations_hash: OCamlOperationListListHash,
                fitness: OCamlList<OCamlBytes>,
                context: OCamlContextHash,
            }
        }
    }
}

unsafe impl<'a> ToOCaml<OCamlBlockHeader> for FfiBlockHeader<'a> {
    fn to_ocaml<'gc>(&self, cr: &'gc mut OCamlRuntime) -> OCaml<'gc, OCamlBlockHeader> {
        ocaml_alloc_record! {
            cr, self {
                shell: OCamlBlockHeaderShellHeader,
                protocol_data: OCamlBytes,
            }
        }
    }
}

unsafe impl<'a> ToOCaml<OCamlOperationShellHeader> for FfiOperationShellHeader<'a> {
    fn to_ocaml<'gc>(&self, cr: &'gc mut OCamlRuntime) -> OCaml<'gc, OCamlOperationShellHeader> {
        ocaml_alloc_record! {
            cr, self {
                branch: OCamlBlockHash,
            }
        }
    }
}

unsafe impl<'a> ToOCaml<OCamlOperation> for FfiOperation<'a> {
    fn to_ocaml<'gc>(&self, cr: &'gc mut OCamlRuntime) -> OCaml<'gc, OCamlOperation> {
        ocaml_alloc_record! {
            cr, self {
                shell: OCamlOperationShellHeader,
                data: OCamlBytes,
            }
        }
    }
}
