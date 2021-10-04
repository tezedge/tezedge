// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

// TODO: remove this file

use std::path::Path;

use ipc::{IpcClient, IpcError};
use slog::{info, warn, Logger};
use tezos_context::IndexApi;
use tezos_context_api::TezosContextConfiguration;
use tezos_wrapper::{
    protocol::ProtocolApi,
    service::{
        ContextGetKeyFromHistoryRequest, ContextGetKeyValuesByPrefixRequest,
        ContextGetTreeByPrefixRequest, JsonEncodeApplyBlockOperationsMetadataParams,
        JsonEncodeApplyBlockResultMetadataParams, NodeMessage, ProtocolMessage,
    },
};

/// Establish connection to existing IPC endpoint (which was created by tezedge node).
/// Begin receiving commands from the tezedge node until `ShutdownCall` command is received.
pub fn process_protocol_commands<Proto: ProtocolApi, P: AsRef<Path>, SDC: Fn(&Logger)>(
    socket_path: P,
    log: &Logger,
    shutdown_callback: SDC,
) -> Result<(), IpcError> {
    let ipc_client: IpcClient<ProtocolMessage, NodeMessage> = IpcClient::new(socket_path);
    let (mut rx, mut tx) = ipc_client.connect()?;
    loop {
        let cmd = rx.receive()?;
        match cmd {
            ProtocolMessage::ApplyBlockCall(request) => {
                let res = Proto::apply_block(request);
                tx.send(&NodeMessage::ApplyBlockResult(res))?;
            }
            ProtocolMessage::AssertEncodingForProtocolDataCall(protocol_hash, protocol_data) => {
                let res = Proto::assert_encoding_for_protocol_data(protocol_hash, protocol_data);
                tx.send(&NodeMessage::AssertEncodingForProtocolDataResult(res))?;
            }
            ProtocolMessage::BeginConstructionCall(request) => {
                let res = Proto::begin_construction(request);
                tx.send(&NodeMessage::BeginConstructionResult(res))?;
            }
            ProtocolMessage::BeginApplicationCall(request) => {
                let res = Proto::begin_application(request);
                tx.send(&NodeMessage::BeginApplicationResult(res))?;
            }
            ProtocolMessage::ValidateOperationCall(request) => {
                let res = Proto::validate_operation(request);
                tx.send(&NodeMessage::ValidateOperationResponse(res))?;
            }
            ProtocolMessage::ProtocolRpcCall(request) => {
                let res = Proto::call_protocol_rpc(request);
                tx.send(&NodeMessage::RpcResponse(res))?;
            }
            ProtocolMessage::HelpersPreapplyOperationsCall(request) => {
                let res = Proto::helpers_preapply_operations(request);
                tx.send(&NodeMessage::HelpersPreapplyResponse(res))?;
            }
            ProtocolMessage::HelpersPreapplyBlockCall(request) => {
                let res = Proto::helpers_preapply_block(request);
                tx.send(&NodeMessage::HelpersPreapplyResponse(res))?;
            }
            ProtocolMessage::ComputePathCall(request) => {
                let res = Proto::compute_path(request);
                tx.send(&NodeMessage::ComputePathResponse(res))?;
            }
            ProtocolMessage::ChangeRuntimeConfigurationCall(params) => {
                let res = Proto::change_runtime_configuration(params);
                tx.send(&NodeMessage::ChangeRuntimeConfigurationResult)?;
            }
            ProtocolMessage::InitProtocolContextCall(params) => {
                let context_config = TezosContextConfiguration {
                    storage: params.storage,
                    genesis: params.genesis,
                    protocol_overrides: params.protocol_overrides,
                    commit_genesis: params.commit_genesis,
                    enable_testchain: params.enable_testchain,
                    readonly: params.readonly,
                    sandbox_json_patch_context: params.patch_context,
                    context_stats_db_path: params.context_stats_db_path,
                };
                // TODO - TE-261: what to do with init protocol result on readonly? init context anyway to get it?
                let res = Proto::init_protocol_context(context_config);
                tx.send(&NodeMessage::InitProtocolContextResult(res))?;
            }
            ProtocolMessage::InitProtocolContextIpcServer(storage_cfg) => {
                // TODO - TE-261: needs better error handling
                match storage_cfg.get_ipc_socket_path() {
                    None => tx.send(&NodeMessage::InitProtocolContextIpcServerResult(Ok(())))?,
                    Some(socket_path) => {
                        match tezos_context::kv_store::readonly_ipc::IpcContextListener::try_new(
                            socket_path.clone(),
                        ) {
                            Ok(mut listener) => {
                                info!(&log, "Listening to context IPC request at {}", socket_path);
                                let log = log.clone();
                                std::thread::Builder::new()
                                    .name("ctx-ipc-lstnr-thread".to_string())
                                    .spawn(move || {
                                        listener.handle_incoming_connections(&log);
                                    })
                                    .map_err(|error| IpcError::ThreadError { reason: error })?;
                                tx.send(&NodeMessage::InitProtocolContextIpcServerResult(Ok(())))?;
                            }
                            Err(err) => {
                                tx.send(&NodeMessage::InitProtocolContextIpcServerResult(Err(
                                    format!("Failed to initialize context IPC server: {:?}", err),
                                )))?;
                            }
                        }
                    }
                }
            }
            ProtocolMessage::GenesisResultDataCall(params) => {
                let res = Proto::genesis_result_data(
                    &params.genesis_context_hash,
                    &params.chain_id,
                    &params.genesis_protocol_hash,
                    params.genesis_max_operations_ttl,
                );
                tx.send(&NodeMessage::CommitGenesisResultData(res))?;
            }
            ProtocolMessage::JsonEncodeApplyBlockResultMetadata(
                JsonEncodeApplyBlockResultMetadataParams {
                    context_hash,
                    metadata_bytes,
                    max_operations_ttl,
                    protocol_hash,
                    next_protocol_hash,
                },
            ) => {
                let res = Proto::apply_block_result_metadata(
                    context_hash,
                    metadata_bytes,
                    max_operations_ttl,
                    protocol_hash,
                    next_protocol_hash,
                );
                tx.send(&NodeMessage::JsonEncodeApplyBlockResultMetadataResponse(
                    res,
                ))?;
            }
            ProtocolMessage::JsonEncodeApplyBlockOperationsMetadata(
                JsonEncodeApplyBlockOperationsMetadataParams {
                    chain_id,
                    operations,
                    operations_metadata_bytes,
                    protocol_hash,
                    next_protocol_hash,
                },
            ) => {
                let res = Proto::apply_block_operations_metadata(
                    chain_id,
                    operations,
                    operations_metadata_bytes,
                    protocol_hash,
                    next_protocol_hash,
                );
                tx.send(&NodeMessage::JsonEncodeApplyBlockOperationsMetadata(res))?;
            }
            ProtocolMessage::ContextGetKeyFromHistory(ContextGetKeyFromHistoryRequest {
                context_hash,
                key,
            }) => {
                match tezos_context::ffi::get_context_index().map_err(|e| IpcError::OtherError {
                    reason: format!("{:?}", e),
                })? {
                    None => tx.send(&NodeMessage::ContextGetKeyFromHistoryResult(Err(
                        "Context index unavailable".to_owned(),
                    )))?,
                    Some(index) => {
                        let key_borrowed: Vec<&str> = key.iter().map(|s| s.as_str()).collect();
                        let result = index
                            .get_key_from_history(&context_hash, &key_borrowed)
                            .map_err(|err| format!("{:?}", err));
                        tx.send(&NodeMessage::ContextGetKeyFromHistoryResult(result))?;
                    }
                }
            }
            ProtocolMessage::ContextGetKeyValuesByPrefix(ContextGetKeyValuesByPrefixRequest {
                context_hash,
                prefix,
            }) => match tezos_context::ffi::get_context_index().map_err(|e| {
                IpcError::OtherError {
                    reason: format!("{:?}", e),
                }
            })? {
                None => tx.send(&NodeMessage::ContextGetKeyFromHistoryResult(Err(
                    "Context index unavailable".to_owned(),
                )))?,
                Some(index) => {
                    let prefix_borrowed: Vec<&str> = prefix.iter().map(|s| s.as_str()).collect();
                    let result = index
                        .get_key_values_by_prefix(&context_hash, &prefix_borrowed)
                        .map_err(|err| format!("{:?}", err));
                    tx.send(&NodeMessage::ContextGetKeyValuesByPrefixResult(result))?;
                }
            },
            ProtocolMessage::ContextGetTreeByPrefix(ContextGetTreeByPrefixRequest {
                context_hash,
                prefix,
                depth,
            }) => match tezos_context::ffi::get_context_index().map_err(|e| {
                IpcError::OtherError {
                    reason: format!("{:?}", e),
                }
            })? {
                None => tx.send(&NodeMessage::ContextGetKeyFromHistoryResult(Err(
                    "Context index unavailable".to_owned(),
                )))?,
                Some(index) => {
                    let prefix_borrowed: Vec<&str> = prefix.iter().map(|s| s.as_str()).collect();
                    let result = index
                        .get_context_tree_by_prefix(&context_hash, &prefix_borrowed, depth)
                        .map_err(|err| format!("{:?}", err));
                    tx.send(&NodeMessage::ContextGetTreeByPrefixResult(result))?;
                }
            },
            ProtocolMessage::ShutdownCall => {
                // we trigger shutdown callback before, returning response
                shutdown_callback(log);

                // return result
                if let Err(e) = tx.send(&NodeMessage::ShutdownResult) {
                    warn!(log, "Failed to send shutdown response"; "reason" => format!("{}", e));
                }

                // drop socket
                drop(tx);
                drop(rx);
                break;
            }
        }
    }

    Ok(())
}
