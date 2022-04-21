// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![allow(clippy::ptr_arg)]

use crate::{
    OCamlApplyBlockExecutionTimestamps, OCamlApplyBlockRequest, OCamlBeginApplicationRequest,
    OCamlBeginConstructionRequest, OCamlBlockHeader, OCamlBlockHeaderShellHeader,
    OCamlBlockPayloadHash, OCamlComputePathRequest, OCamlContextGetKeyFromHistoryRequest,
    OCamlContextGetKeyValuesByPrefixRequest, OCamlContextGetTreeByPrefixRequest,
    OCamlCycleRollsOwnerSnapshot, OCamlDumpContextRequest, OCamlGenesisChain,
    OCamlGenesisResultDataParams, OCamlHelpersPreapplyBlockRequest, OCamlInitProtocolContextParams,
    OCamlJsonEncodeApplyBlockOperationsMetadataParams,
    OCamlJsonEncodeApplyBlockResultMetadataParams, OCamlOperation, OCamlOperationShellHeader,
    OCamlPatchContext, OCamlProtocolMessage, OCamlProtocolOverrides, OCamlProtocolRpcRequest,
    OCamlRestoreContextRequest, OCamlRpcRequest, OCamlTezosContextConfiguration,
    OCamlTezosContextIrminStorageConfiguration, OCamlTezosContextStorageConfiguration,
    OCamlTezosContextTezedgeOnDiskBackendOptions, OCamlTezosRuntimeConfiguration,
    OCamlTezosRuntimeLogLevel, OCamlValidateOperationRequest,
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
    BlockHash, BlockMetadataHash, BlockPayloadHash, ChainId, ContextHash, Hash, OperationHash,
    OperationListListHash, OperationMetadataHash, OperationMetadataListListHash, ProtocolHash,
};
use ocaml_interop::{
    impl_to_ocaml_polymorphic_variant, impl_to_ocaml_record, impl_to_ocaml_variant,
    ocaml_alloc_record, ocaml_alloc_variant, OCaml, OCamlBytes, OCamlFloat, OCamlInt, OCamlInt32,
    OCamlInt64, OCamlList, OCamlRuntime, ToOCaml,
};
use tezos_api::ffi::{
    ApplyBlockExecutionTimestamps, ApplyBlockRequest, ApplyBlockResponse, BeginApplicationRequest,
    BeginConstructionRequest, ComputePathRequest, CycleRollsOwnerSnapshot, ForkingTestchainData,
    HelpersPreapplyBlockRequest, PrevalidatorWrapper, ProtocolRpcRequest, RpcMethod, RpcRequest,
    TezosRuntimeConfiguration, TezosRuntimeLogLevel, ValidateOperationRequest,
};
use tezos_context_api::{
    ContextKvStoreConfiguration, GenesisChain, PatchContext, ProtocolOverrides,
    TezosContextConfiguration, TezosContextIrminStorageConfiguration,
    TezosContextStorageConfiguration, TezosContextTezEdgeStorageConfiguration,
    TezosContextTezedgeOnDiskBackendOptions,
};
use tezos_messages::p2p::encoding::{
    fitness::Fitness,
    prelude::{BlockHeader, Operation},
};
use tezos_protocol_ipc_messages::{
    ContextGetKeyFromHistoryRequest, ContextGetKeyValuesByPrefixRequest,
    ContextGetTreeByPrefixRequest, DumpContextRequest, GenesisResultDataParams,
    InitProtocolContextParams, JsonEncodeApplyBlockOperationsMetadataParams,
    JsonEncodeApplyBlockResultMetadataParams, ProtocolMessage, RestoreContextRequest,
};

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
to_ocaml_hash!(OCamlBlockPayloadHash, BlockPayloadHash);
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

impl_to_ocaml_record! {
    TezosContextTezedgeOnDiskBackendOptions => OCamlTezosContextTezedgeOnDiskBackendOptions {
        base_path: String,
        startup_check: bool,
    }
}

impl_to_ocaml_variant! {
    ContextKvStoreConfiguration => OCamlContextKvStoreConfiguration {
        ContextKvStoreConfiguration::ReadOnlyIpc,
        ContextKvStoreConfiguration::InMem(options: OCamlTezosContextTezedgeOnDiskBackendOptions),
        ContextKvStoreConfiguration::OnDisk(options: OCamlTezosContextTezedgeOnDiskBackendOptions),
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
            timestamp: block_header.timestamp().into(),
            validation_passes: block_header.validation_pass() as i32,
            operations_hash,
            fitness: block_header.fitness(),
            context,
        }
    }
}

impl<'a> From<&'a BlockHeader> for FfiBlockHeader<'a> {
    fn from(block_header: &'a BlockHeader) -> Self {
        let shell = FfiBlockHeaderShellHeader::from(block_header);
        Self {
            shell,
            protocol_data: block_header.protocol_data().as_ref(),
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
            data: operation.data().as_ref(),
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
        execution_timestamps: OCamlApplyBlockExecutionTimestamps,
    }
}

impl_to_ocaml_record! {
    ApplyBlockExecutionTimestamps => OCamlApplyBlockExecutionTimestamps {
        apply_start_t: OCamlFloat,
        operations_decoding_start_t: OCamlFloat,
        operations_decoding_end_t: OCamlFloat,
        operations_application_timestamps: OCamlList<OCamlList<(OCamlFloat, OCamlFloat)>>,
        operations_metadata_encoding_start_t: OCamlFloat,
        operations_metadata_encoding_end_t: OCamlFloat,
        begin_application_start_t: OCamlFloat,
        begin_application_end_t: OCamlFloat,
        finalize_block_start_t: OCamlFloat,
        finalize_block_end_t: OCamlFloat,
        collect_new_rolls_owner_snapshots_start_t: OCamlFloat,
        collect_new_rolls_owner_snapshots_end_t: OCamlFloat,
        commit_start_t: OCamlFloat,
        commit_end_t: OCamlFloat,
        apply_end_t: OCamlFloat,
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
        predecessor_hash: OCamlBlockHash,
        protocol_data: Option<OCamlBytes>,
        predecessor_block_metadata_hash: Option<OCamlBlockMetadataHash>,
        predecessor_ops_metadata_hash: Option<OCamlOperationMetadataListListHash>,
    }
}

impl_to_ocaml_record! {
    PrevalidatorWrapper => OCamlPrevalidatorWrapper {
        chain_id: OCamlChainId,
        protocol: OCamlProtocolHash,
        context_fitness: Option<OCamlList<OCamlBytes>>,
        predecessor: OCamlBlockHash,
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
        predecessor_max_operations_ttl: OCamlInt,
    }
}

impl_to_ocaml_record! {
    ComputePathRequest => OCamlComputePathRequest {
        operations: OCamlList<OCamlList<OCamlOperationHash>>
    }
}

impl_to_ocaml_record! {
    GenesisResultDataParams => OCamlGenesisResultDataParams {
        genesis_context_hash: OCamlContextHash,
        chain_id: OCamlChainId,
        genesis_protocol_hash: OCamlProtocolHash,
        genesis_max_operations_ttl: OCamlInt => *genesis_max_operations_ttl as i32,
    }
}

impl_to_ocaml_record! {
    GenesisChain => OCamlGenesisChain {
        time: String,
        block: String,
        protocol: String,
    }
}

impl_to_ocaml_record! {
    ProtocolOverrides => OCamlProtocolOverrides{
        user_activated_upgrades: OCamlList<(OCamlInt32, String)>,
        user_activated_protocol_overrides: OCamlList<(String, String)>,
    }
}

impl_to_ocaml_record! {
    PatchContext => OCamlPatchContext {
        key: String,
        json: String,
    }
}

impl_to_ocaml_record! {
    InitProtocolContextParams => OCamlInitProtocolContextParams {
        storage: OCamlTezosContextStorageConfiguration,
        genesis: OCamlGenesisChain,
        //genesis_max_operations_ttl: OCamlInt => *genesis_max_operations_ttl as i32,
        protocol_overrides: OCamlProtocolOverrides,
        commit_genesis: bool,
        enable_testchain: bool,
        readonly: bool,
        patch_context: Option<OCamlPatchContext>,
        context_stats_db_path: Option<String> =>
            context_stats_db_path.clone().map(|path| path.to_string_lossy().to_string()),
    }
}

impl_to_ocaml_variant! {
    TezosRuntimeLogLevel => OCamlTezosRuntimeLogLevel {
        TezosRuntimeLogLevel::App,
        TezosRuntimeLogLevel::Error,
        TezosRuntimeLogLevel::Warning,
        TezosRuntimeLogLevel::Info,
        TezosRuntimeLogLevel::Debug,
    }
}

impl_to_ocaml_record! {
    TezosRuntimeConfiguration => OCamlTezosRuntimeConfiguration {
        log_enabled: bool,
        log_level: Option<OCamlTezosRuntimeLogLevel>,
    }
}

impl_to_ocaml_record! {
    JsonEncodeApplyBlockResultMetadataParams => OCamlJsonEncodeApplyBlockResultMetadataParams {
        context_hash: OCamlContextHash,
        metadata_bytes: OCamlBytes,
        max_operations_ttl: OCamlInt,
        protocol_hash: OCamlProtocolHash,
        next_protocol_hash: OCamlProtocolHash,
    }
}

fn convert_operations(operations: &Vec<Vec<Operation>>) -> Vec<Vec<FfiOperation>> {
    let converted: Vec<Vec<FfiOperation>> = operations
        .iter()
        .map(|ops| ops.iter().map(FfiOperation::from).collect())
        .collect();
    converted
}

impl_to_ocaml_record! {
    JsonEncodeApplyBlockOperationsMetadataParams => OCamlJsonEncodeApplyBlockOperationsMetadataParams {
        chain_id: OCamlChainId,
        operations: OCamlList<OCamlList<OCamlOperation>> => convert_operations(operations),
        operations_metadata_bytes: OCamlList<OCamlList<OCamlBytes>>,
        protocol_hash: OCamlProtocolHash,
        next_protocol_hash: OCamlProtocolHash,
    }
}

impl_to_ocaml_record! {
    ContextGetKeyFromHistoryRequest => OCamlContextGetKeyFromHistoryRequest {
        context_hash: OCamlContextHash,
        key: OCamlList<String>,
    }
}

impl_to_ocaml_record! {
    ContextGetKeyValuesByPrefixRequest => OCamlContextGetKeyValuesByPrefixRequest {
        context_hash: OCamlContextHash,
        prefix: OCamlList<String>,
    }
}

impl_to_ocaml_record! {
    ContextGetTreeByPrefixRequest => OCamlContextGetTreeByPrefixRequest {
        context_hash: OCamlContextHash,
        prefix: OCamlList<String>,
        depth: Option<OCamlInt> => (*depth).map(|depth| depth as i32),
    }
}

impl_to_ocaml_record! {
    DumpContextRequest => OCamlDumpContextRequest {
        context_hash: OCamlContextHash,
        dump_into_path: String,
    }
}

impl_to_ocaml_record! {
    RestoreContextRequest => OCamlRestoreContextRequest {
        expected_context_hash: OCamlContextHash,
        restore_from_path: String,
        nb_context_elements: OCamlInt,
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
                fitness: OCamlList<OCamlBytes> => Fitness::as_ref(fitness),
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

impl_to_ocaml_polymorphic_variant! {
    ProtocolMessage => OCamlProtocolMessage {
        ProtocolMessage::ApplyBlockCall(req: OCamlApplyBlockRequest),
        ProtocolMessage::AssertEncodingForProtocolDataCall(hash: OCamlProtocolHash, data: OCamlBytes),
        ProtocolMessage::BeginApplicationCall(req: OCamlBeginApplicationRequest),
        ProtocolMessage::BeginConstructionForPrevalidationCall(req: OCamlBeginConstructionRequest),
        ProtocolMessage::ValidateOperationForPrevalidationCall(req: OCamlValidateOperationRequest),
        ProtocolMessage::BeginConstructionForMempoolCall(req: OCamlBeginConstructionRequest),
        ProtocolMessage::ValidateOperationForMempoolCall(req: OCamlValidateOperationRequest),
        ProtocolMessage::ProtocolRpcCall(req: OCamlProtocolRpcRequest),
        ProtocolMessage::HelpersPreapplyOperationsCall(req: OCamlProtocolRpcRequest),
        ProtocolMessage::HelpersPreapplyBlockCall(req: OCamlHelpersPreapplyBlockRequest),
        ProtocolMessage::ComputePathCall(req: OCamlComputePathRequest),
        ProtocolMessage::ChangeRuntimeConfigurationCall(cfg: OCamlTezosRuntimeConfiguration),
        ProtocolMessage::InitProtocolContextCall(params: OCamlInitProtocolContextParams),
        ProtocolMessage::InitProtocolContextIpcServer(cfg: OCamlTezosContextStorageConfiguration),
        ProtocolMessage::GenesisResultDataCall(params: OCamlGenesisResultDataParams),
        ProtocolMessage::JsonEncodeApplyBlockResultMetadata(params: OCamlJsonEncodeApplyBlockResultMetadataParams),
        ProtocolMessage::JsonEncodeApplyBlockOperationsMetadata(params: OCamlJsonEncodeApplyBlockOperationsMetadataParams),
        ProtocolMessage::ContextGetKeyFromHistory(req: OCamlContextGetKeyFromHistoryRequest),
        ProtocolMessage::ContextGetKeyValuesByPrefix(req: OCamlContextGetKeyValuesByPrefixRequest),
        ProtocolMessage::ContextGetTreeByPrefix(req: OCamlContextGetTreeByPrefixRequest),
        ProtocolMessage::DumpContext(req: OCamlDumpContextRequest),
        ProtocolMessage::RestoreContext(req: OCamlRestoreContextRequest),
        ProtocolMessage::ContextGetLatestContextHashes(req: OCamlInt),
        ProtocolMessage::Ping,
        ProtocolMessage::ShutdownCall,
    }
}
