// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;
use std::sync::Once;

use crypto::hash::ChainId;
use crypto::hash::{ContextHash, ProtocolHash};
use ocaml_interop::{OCaml, OCamlRuntime, ToOCaml};

use tezos_api::ffi::*;
use tezos_context_api::TezosContextConfiguration;
use tezos_conv::*;
use tezos_messages::p2p::encoding::operation::Operation;

use crate::runtime;

type TzResult<T> = Result<T, OCamlTezosErrorTrace>;

mod tezos_ffi {
    use ocaml_interop::{ocaml, OCamlBytes, OCamlInt, OCamlList};
    use tezos_conv::*;

    use super::TzResult;

    ocaml! {
        pub fn apply_block(
            apply_block_request: OCamlApplyBlockRequest
        ) -> TzResult<OCamlApplyBlockResponse>;
        pub fn begin_application(
            begin_application_request: OCamlBeginApplicationRequest
        ) -> TzResult<OCamlBeginApplicationResponse>;
        pub fn begin_construction(
            begin_construction_request: OCamlBeginConstructionRequest
        ) -> TzResult<OCamlPrevalidatorWrapper>;
        pub fn validate_operation(
            validate_operation_request: OCamlValidateOperationRequest
        ) -> TzResult<OCamlValidateOperationResponse>;
        pub fn call_protocol_rpc(
            request: OCamlProtocolRpcRequest
        ) -> Result<OCamlProtocolRpcResponse, OCamlProtocolRpcError>;
        pub fn helpers_preapply_operations(
            request: OCamlProtocolRpcRequest
        ) -> TzResult<OCamlHelpersPreapplyResponse>;
        pub fn helpers_preapply_block(
            request: OCamlHelpersPreapplyBlockRequest
        ) -> TzResult<OCamlHelpersPreapplyResponse>;
        pub fn change_runtime_configuration(
            log_enabled: bool, debug_mode: bool, compute_context_action_tree_hashes: bool,
        );
        pub fn init_protocol_context(
            context_config: OCamlTezosContextConfiguration
        ) -> TzResult<(OCamlList<OCamlBytes>, Option<OCamlBytes>)>;
        pub fn genesis_result_data(
            context_hash: OCamlBytes,
            chain_id: OCamlBytes,
            protocol_hash: OCamlBytes,
            genesis_max_operations_ttl: OCamlInt
        ) -> TzResult<(OCamlBytes, OCamlBytes, OCamlList<OCamlList<OCamlBytes>>)>;
        pub fn decode_context_data(
            protocol_hash: OCamlBytes,
            key: OCamlList<OCamlBytes>,
            data: OCamlBytes
        ) -> TzResult<Option<OCamlBytes>>;
        pub fn compute_path(
            request: OCamlList<OCamlList<OCamlOperationHash>>
        ) -> TzResult<OCamlList<FfiPath>>;
        pub fn assert_encoding_for_protocol_data(
            protocol_hash: OCamlProtocolHash,
            protocol_data: OCamlBytes
        ) -> TzResult<()>;
        pub fn apply_block_result_metadata(
            context_hash: OCamlBytes,
            metadata_bytes: OCamlBytes,
            max_operations_ttl: OCamlInt,
            protocol_hash: OCamlBytes,
            next_protocol_hash: OCamlBytes,
        ) -> TzResult<OCamlBytes>;
        pub fn apply_block_operations_metadata(
            chain_id: OCamlBytes,
            operations: OCamlList<OCamlList<OCamlOperation>>,
            operations_metadata_bytes: OCamlList<OCamlList<OCamlBytes>>,
            protocol_hash: OCamlBytes,
            next_protocol_hash: OCamlBytes,
        ) -> TzResult<OCamlBytes>;
    }
}

/// Initializes the ocaml runtime and the tezos-ffi callback mechanism.
pub fn setup() -> OCamlRuntime {
    static INIT: Once = Once::new();
    let ocaml_runtime = OCamlRuntime::init();

    INIT.call_once(|| {
        tezos_context::ffi::initialize_callbacks();
    });

    ocaml_runtime
}

pub fn shutdown() {
    runtime::shutdown()
}

pub fn change_runtime_configuration(
    settings: TezosRuntimeConfiguration,
) -> Result<(), TezosRuntimeConfigurationError> {
    runtime::execute(move |rt: &mut OCamlRuntime| {
        tezos_ffi::change_runtime_configuration(
            rt,
            &OCaml::of_bool(settings.log_enabled),
            &OCaml::of_bool(settings.debug_mode),
            &OCaml::of_bool(settings.compute_context_action_tree_hashes),
        );
    })
    .map_err(
        |p| TezosRuntimeConfigurationError::ChangeConfigurationError {
            message: p.to_string(),
        },
    )
}

pub fn init_protocol_context(
    context_config: TezosContextConfiguration,
) -> Result<InitProtocolContextResult, TezosStorageInitError> {
    runtime::execute(move |rt: &mut OCamlRuntime| {
        let context_config = context_config.to_boxroot(rt);
        let result = tezos_ffi::init_protocol_context(rt, &context_config);
        let result = rt.get(&result).to_result();

        match result {
            Ok(result) => {
                let (supported_protocol_hashes, genesis_commit_hash): (
                    Vec<RustBytes>,
                    Option<RustBytes>,
                ) = result.to_rust();
                let supported_protocol_hashes = supported_protocol_hashes
                    .into_iter()
                    .map(ProtocolHash::try_from)
                    .collect::<Result<_, _>>()?;
                let genesis_commit_hash = genesis_commit_hash
                    .map(|bytes| ContextHash::try_from(bytes.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?;
                Ok(InitProtocolContextResult {
                    supported_protocol_hashes,
                    genesis_commit_hash,
                })
            }
            Err(e) => Err(TezosStorageInitError::from(e.to_rust::<TezosErrorTrace>())),
        }
    })
    .unwrap_or_else(|p| {
        Err(TezosStorageInitError::InitializeError {
            message: p.to_string(),
        })
    })
}

pub fn genesis_result_data(
    context_hash: RustBytes,
    chain_id: RustBytes,
    protocol_hash: RustBytes,
    genesis_max_operations_ttl: u16,
) -> Result<CommitGenesisResult, GetDataError> {
    runtime::execute(move |rt: &mut OCamlRuntime| {
        let context_hash = context_hash.to_boxroot(rt);
        let chain_id = chain_id.to_boxroot(rt);
        let protocol_hash = protocol_hash.to_boxroot(rt);
        let genesis_max_operations_ttl = OCaml::of_i32(genesis_max_operations_ttl as i32);

        let result = tezos_ffi::genesis_result_data(
            rt,
            &context_hash,
            &chain_id,
            &protocol_hash,
            &genesis_max_operations_ttl,
        );
        let result = rt.get(&result).to_result();
        match result {
            Ok(result) => {
                let (
                    block_header_proto_json,
                    block_header_proto_metadata_bytes,
                    operations_proto_metadata_bytes,
                ) = result.to_rust();
                Ok(CommitGenesisResult {
                    block_header_proto_json,
                    block_header_proto_metadata_bytes,
                    operations_proto_metadata_bytes,
                })
            }
            Err(e) => Err(GetDataError::from(e.to_rust::<TezosErrorTrace>())),
        }
    })
    .unwrap_or_else(|p| {
        Err(GetDataError::ReadError {
            message: p.to_string(),
        })
    })
}

macro_rules! call_helper {
    (tezos_ffi::$f:ident($request:ident)) => {
        runtime::execute(move |rt: &mut OCamlRuntime| {
            let ocaml_request = $request.to_boxroot(rt);
            let result = tezos_ffi::$f(rt, &ocaml_request);
            let result = rt.get(&result).to_result();
            match result {
                Ok(response) => Ok(response.to_rust()),
                Err(e) => Err(CallError::from(e.to_rust::<TezosErrorTrace>())),
            }
        })
        .unwrap_or_else(|p| {
            Err(CallError::FailedToCall {
                error_id: "@OCamlBlockPanic".to_owned(),
                trace_message: p.to_string(),
            })
        })
    };
}

/// Applies block to context
pub fn apply_block(request: ApplyBlockRequest) -> Result<ApplyBlockResponse, CallError> {
    call_helper!(tezos_ffi::apply_block(request))
}

/// Begin construction initializes prevalidator and context for new operations based on current head
pub fn begin_application(
    request: BeginApplicationRequest,
) -> Result<BeginApplicationResponse, CallError> {
    call_helper!(tezos_ffi::begin_application(request))
}

/// Begin construction initializes prevalidator and context for new operations based on current head
pub fn begin_construction(
    request: BeginConstructionRequest,
) -> Result<PrevalidatorWrapper, CallError> {
    call_helper!(tezos_ffi::begin_construction(request))
}

/// Validate operation - used with prevalidator for validation of operation
pub fn validate_operation(
    request: ValidateOperationRequest,
) -> Result<ValidateOperationResponse, CallError> {
    call_helper!(tezos_ffi::validate_operation(request))
}

pub fn call_protocol_rpc(
    request: ProtocolRpcRequest,
) -> Result<ProtocolRpcResponse, ProtocolRpcError> {
    runtime::execute(move |rt: &mut OCamlRuntime| {
        let request = request.to_boxroot(rt);
        let result = tezos_ffi::call_protocol_rpc(rt, &request);
        // TODO: should call_protocol_rpc be Result<_, OCamlErrorTrace> instead?
        // looks like not, but verify and add a catch-all for unhandled cases
        rt.get(&result).to_rust()
    })
    .unwrap_or_else(|p| Err(ProtocolRpcError::FailedToCallProtocolRpc(p.to_string())))
}

/// Call helpers_preapply_operations shell service
pub fn helpers_preapply_operations(
    request: ProtocolRpcRequest,
) -> Result<HelpersPreapplyResponse, CallError> {
    call_helper!(tezos_ffi::helpers_preapply_operations(request))
}

/// Call helpers_preapply_block shell service
pub fn helpers_preapply_block(
    request: HelpersPreapplyBlockRequest,
) -> Result<HelpersPreapplyResponse, CallError> {
    call_helper!(tezos_ffi::helpers_preapply_block(request))
}

/// Call compute path
pub fn compute_path(request: ComputePathRequest) -> Result<ComputePathResponse, CallError> {
    runtime::execute(move |rt: &mut OCamlRuntime| {
        let operations = request.operations.to_boxroot(rt);
        let result = tezos_ffi::compute_path(rt, &operations);
        let result = rt.get(&result).to_result();
        match result {
            Ok(response) => {
                let operations_hashes_path: Vec<FfiPath> = response.to_rust();
                let operations_hashes_path = operations_hashes_path
                    .into_iter()
                    .map(|path| {
                        let mut res = Vec::new();
                        let mut path = path;
                        loop {
                            use tezos_messages::p2p::encoding::operations_for_blocks::{
                                Path, PathItem,
                            };
                            match path {
                                FfiPath::Right(right) => {
                                    res.push(PathItem::right(right.left));
                                    path = right.path;
                                }
                                FfiPath::Left(left) => {
                                    res.push(PathItem::left(left.right));
                                    path = left.path;
                                }
                                FfiPath::Op => {
                                    return Path(res);
                                }
                            }
                        }
                    })
                    .collect();
                Ok(ComputePathResponse {
                    operations_hashes_path,
                })
            }
            Err(e) => Err(CallError::from(e.to_rust::<TezosErrorTrace>())),
        }
    })
    .unwrap_or_else(|p| {
        Err(CallError::FailedToCall {
            error_id: "@exception".to_owned(),
            trace_message: p.to_string(),
        })
    })
}

pub fn decode_context_data(
    protocol_hash: RustBytes,
    key: Vec<String>,
    data: RustBytes,
) -> Result<Option<String>, ContextDataError> {
    runtime::execute(move |rt: &mut OCamlRuntime| {
        let protocol_hash = protocol_hash.to_boxroot(rt);
        let key_list = key.to_boxroot(rt);
        let data = data.to_boxroot(rt);

        let result = tezos_ffi::decode_context_data(rt, &protocol_hash, &key_list, &data);
        let result = rt.get(&result).to_result();

        match result {
            Ok(decoded_data) => Ok(decoded_data.to_rust()),
            Err(e) => Err(ContextDataError::from(e.to_rust::<TezosErrorTrace>())),
        }
    })
    .unwrap_or_else(|p| {
        Err(ContextDataError::DecodeError {
            message: p.to_string(),
        })
    })
}

pub fn assert_encoding_for_protocol_data(
    protocol_hash: RustBytes,
    protocol_data: RustBytes,
) -> Result<(), ProtocolDataError> {
    runtime::execute(move |rt: &mut OCamlRuntime| {
        let protocol_hash = ProtocolHash::try_from(protocol_hash)?;
        let protocol_hash = protocol_hash.to_boxroot(rt);
        let data = protocol_data.to_boxroot(rt);

        let result = tezos_ffi::assert_encoding_for_protocol_data(rt, &protocol_hash, &data);
        let result = rt.get(&result).to_result();

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(ProtocolDataError::from(e.to_rust::<TezosErrorTrace>())),
        }
    })
    .unwrap_or_else(|p| {
        Err(ProtocolDataError::DecodeError {
            message: p.to_string(),
        })
    })
}

pub fn apply_block_result_metadata(
    context_hash: ContextHash,
    metadata_bytes: RustBytes,
    max_operations_ttl: i32,
    protocol_hash: ProtocolHash,
    next_protocol_hash: ProtocolHash,
) -> Result<String, FfiJsonEncoderError> {
    runtime::execute(move |rt: &mut OCamlRuntime| {
        let context_hash = context_hash.as_ref().to_boxroot(rt);
        let metadata_bytes = metadata_bytes.to_boxroot(rt);
        let max_operations_ttl = OCaml::of_i32(max_operations_ttl as i32);
        let protocol_hash = protocol_hash.as_ref().to_boxroot(rt);
        let next_protocol_hash = next_protocol_hash.as_ref().to_boxroot(rt);

        let result = tezos_ffi::apply_block_result_metadata(
            rt,
            &context_hash,
            &metadata_bytes,
            &max_operations_ttl,
            &protocol_hash,
            &next_protocol_hash,
        );
        let result = rt.get(&result).to_result();

        match result {
            Ok(s) => Ok(s.to_rust()),
            Err(e) => Err(FfiJsonEncoderError::from(e.to_rust::<TezosErrorTrace>())),
        }
    })
    .unwrap_or_else(|p| {
        Err(FfiJsonEncoderError::EncodeError {
            message: p.to_string(),
        })
    })
}

pub fn apply_block_operations_metadata(
    chain_id: ChainId,
    operations: Vec<Vec<Operation>>,
    operations_metadata_bytes: Vec<Vec<RustBytes>>,
    protocol_hash: ProtocolHash,
    next_protocol_hash: ProtocolHash,
) -> Result<String, FfiJsonEncoderError> {
    runtime::execute(move |rt: &mut OCamlRuntime| {
        let chain_id = chain_id.as_ref().to_boxroot(rt);
        let ffi_operations: Vec<Vec<FfiOperation>> = operations
            .iter()
            .map(|v| v.iter().map(|op| FfiOperation::from(op)).collect())
            .collect();
        let ffi_operations = ffi_operations.to_boxroot(rt);
        let operations_metadata_bytes = operations_metadata_bytes.to_boxroot(rt);
        let protocol_hash = protocol_hash.as_ref().to_boxroot(rt);
        let next_protocol_hash = next_protocol_hash.as_ref().to_boxroot(rt);

        let result = tezos_ffi::apply_block_operations_metadata(
            rt,
            &chain_id,
            &ffi_operations,
            &operations_metadata_bytes,
            &protocol_hash,
            &next_protocol_hash,
        );
        let result = rt.get(&result).to_result();

        match result {
            Ok(s) => Ok(s.to_rust()),
            Err(e) => Err(FfiJsonEncoderError::from(e.to_rust::<TezosErrorTrace>())),
        }
    })
    .unwrap_or_else(|p| {
        Err(FfiJsonEncoderError::EncodeError {
            message: p.to_string(),
        })
    })
}
