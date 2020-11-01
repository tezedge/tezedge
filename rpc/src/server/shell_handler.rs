// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use bytes::buf::BufExt;
use hyper::{Body, Request};
use serde::Serialize;

use crypto::hash::HashType;
use shell::shell_channel::BlockApplied;
use tezos_api::ffi::ProtocolRpcError;
use tezos_messages::ts_to_rfc3339;
use tezos_wrapper::service::{ProtocolError, ProtocolServiceError};

use crate::{
    empty,
    encoding::{
        base_types::*,
        monitor::BootstrapInfo,
    },
    make_json_response,
    make_json_stream_response,
    result_option_to_json_response,
    result_to_json_response,
    ServiceResult,
    services,
};
use crate::helpers::create_ffi_json_request;
use crate::server::{HasSingleValue, HResult, Params, Query, RpcServiceEnvironment};
use crate::services::base_services;

#[derive(Serialize)]
pub struct ErrorMessage {
    error_type: String,
    message: String,
}

pub async fn bootstrapped(_: Request<Body>, _: Params, _: Query, env: RpcServiceEnvironment) -> HResult {
    let state_read = env.state().read().unwrap();

    let bootstrap_info = match state_read.current_head().as_ref() {
        Some(current_head) => {
            let current_head: BlockApplied = current_head.clone();
            let block = HashType::BlockHash.bytes_to_string(&current_head.header().hash);
            let timestamp = ts_to_rfc3339(current_head.header().header.timestamp());
            BootstrapInfo::new(block.into(), TimeStamp::Rfc(timestamp))
        }
        None => BootstrapInfo::new(String::new().into(), TimeStamp::Integral(0))
    };

    make_json_response(&bootstrap_info)
}

pub async fn commit_hash(_: Request<Body>, _: Params, _: Query, _: RpcServiceEnvironment) -> HResult {
    let resp = &UniString::from(env!("GIT_HASH"));
    make_json_response(&resp)
}

pub async fn active_chains(_: Request<Body>, _: Params, _: Query, _: RpcServiceEnvironment) -> HResult {
    empty()
}

pub async fn protocols(_: Request<Body>, _: Params, _: Query, _: RpcServiceEnvironment) -> HResult {
    empty()
}

pub async fn valid_blocks(_: Request<Body>, _: Params, _: Query, _: RpcServiceEnvironment) -> HResult {
    empty()
}

pub async fn head_chain(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();

    if chain_id == "main" {
        make_json_stream_response(base_services::get_current_head_monitor_header(env.state())?.unwrap())
    } else {
        // TODO: implement... 
        empty()
    }
}

pub async fn chains_block_id(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();

    use crate::encoding::chain::BlockInfo;
    if chain_id == "main" {
        if block_id == "head" {
            result_option_to_json_response(base_services::get_full_current_head(env.state()).map(|res| res.map(BlockInfo::from)), env.log())
        } else {
            result_option_to_json_response(base_services::get_full_block(block_id, env.persistent_storage(), env.state()).map(|res| res.map(BlockInfo::from)), env.log())
        }
    } else {
        empty()
    }
}

pub async fn chains_block_id_header(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();

    if chain_id == "main" {
        if block_id == "head" {
            result_option_to_json_response(base_services::get_current_head_header(env.state()).map(|res| res), env.log())
        } else {
            result_option_to_json_response(base_services::get_block_header(block_id, env.persistent_storage(), env.state()).map(|res| res), env.log())
        }
    } else {
        empty()
    }
}

pub async fn chains_block_id_header_shell(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();

    if chain_id == "main" {
        if block_id == "head" {
            result_option_to_json_response(base_services::get_current_head_shell_header(env.state()).map(|res| res), env.log())
        } else {
            result_option_to_json_response(base_services::get_block_shell_header(block_id, env.persistent_storage(), env.state()).map(|res| res), env.log())
        }
    } else {
        empty()
    }
}

pub async fn context_raw_bytes(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let block_id = params.get_str("block_id").unwrap();
    let prefix = params.get_str("any");
    result_option_to_json_response(base_services::get_context_raw_bytes(block_id, prefix, env.persistent_storage(), env.tezedge_context(), env.state(), env.log()), env.log())
}

pub async fn mempool_pending_operations(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();

    if chain_id == "main" {
        result_to_json_response(
            services::mempool_services::get_pending_operations(env.state(), env.log()),
            env.log(),
        )
    } else {
        unimplemented!("not implemented yet")
    }
}

pub async fn inject_operation(req: Request<Body>, _: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let operation_data_raw = hyper::body::aggregate(req).await?;
    let operation_data: String = serde_json::from_reader(&mut operation_data_raw.reader())?;

    let shell_channel = env.shell_channel();

    result_to_json_response(
        services::mempool_services::inject_operation(
            &operation_data,
            &env,
            shell_channel.clone(),
        ),
        env.log(),
    )
}

pub async fn inject_block(req: Request<Body>, _: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let body = hyper::body::to_bytes(req.into_body()).await?;
    let body = String::from_utf8(body.to_vec())?;

    let shell_channel = env.shell_channel();

    result_to_json_response(
        services::mempool_services::inject_block(&body, &env, shell_channel.clone()),
        env.log(),
    )
}

pub async fn get_block_protocols(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let _chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();


    result_to_json_response(
        base_services::get_block_protocols(block_id, env.persistent_storage(), env.state()),
        env.log(),
    )
}

pub async fn get_block_hash(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let _chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();

    result_to_json_response(
        base_services::get_block_hash(block_id, env.persistent_storage(), env.state()),
        env.log(),
    )
}

pub async fn get_chain_id(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    // this chain_id (e.g. main) reporesents the "alias" for the actial base58 encoded id (e.g. NetXdQprcVkpaWU)
    let _chain_id = params.get_str("chain_id").unwrap();

    result_to_json_response(
        base_services::get_chain_id(env.state()),
        env.log(),
    )
}

pub async fn get_block_operation_hashes(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let _chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();


    result_to_json_response(
        base_services::get_block_operation_hashes(block_id, env.persistent_storage(), env.state()),
        env.log(),
    )
}

pub async fn live_blocks(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_param = params.get_str("chain_id").unwrap();
    let block_param = params.get_str("block_id").unwrap();

    result_to_json_response(
        services::base_services::live_blocks(chain_param, block_param, &env),
        env.log(),
    )
}

pub async fn preapply_operations(req: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_param = params.get_str("chain_id").unwrap();
    let block_param = params.get_str("block_id").unwrap();

    let json_request = create_ffi_json_request(req).await?;

    result_to_json_response(
        services::protocol::preapply_operations(chain_param, block_param, json_request, &env),
        env.log(),
    )
}

pub async fn preapply_block(req: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_param = params.get_str("chain_id").unwrap();
    let block_param = params.get_str("block_id").unwrap();

    let json_request = create_ffi_json_request(req).await?;

    // launcher - we need the error from preapply
    match services::protocol::preapply_block(chain_param, block_param, json_request, &env) {
        Ok(resp) => result_to_json_response(Ok(resp), env.log()),
        Err(e) => {
            if let Some(err) = e.as_fail().downcast_ref::<ProtocolServiceError>() {
                if let ProtocolServiceError::ProtocolError { reason: ProtocolError::ProtocolRpcError { reason: ProtocolRpcError::FailedToCallProtocolRpc { message } } } = err {
                    return make_json_response(&ErrorMessage {
                        error_type: "ocaml".to_string(),
                        message: message.to_string(),
                    });
                }
            }
            empty()
        }
    }
}

pub async fn node_version(_: Request<Body>, _: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    result_to_json_response(
        base_services::get_node_version(env.network_version()),
        env.log(),
    )
}