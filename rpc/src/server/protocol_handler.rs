// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{make_response_with_status_and_json_string, not_found};
use hyper::{Body, Request};
use slog::warn;

use crate::helpers::{create_rpc_request, parse_block_hash, parse_chain_id, RpcServiceError};
use crate::server::{HasSingleValue, Params, Query, RpcServiceEnvironment};
use crate::services::protocol::{ContextParamsError, RightsError, VotesError};
use crate::{
    handle_rpc_service_error, parse_block_hash_or_fail, required_param, result_to_json_response,
    services, ServiceResult,
};
use std::sync::Arc;

pub async fn context_constants(
    req: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id_param = required_param!(params, "chain_id")?;
    let chain_id = parse_chain_id(chain_id_param, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    // try to call our implementation
    let result =
        services::protocol::get_context_constants_just_for_rpc(&chain_id, &block_hash, &env);

    // fallback, if protocol is not supported, we trigger rpc protocol router
    if let Err(ContextParamsError::UnsupportedProtocolError { .. }) = result {
        result_to_json_response(
            services::protocol::call_protocol_rpc(
                chain_id_param,
                chain_id,
                block_hash,
                create_rpc_request(req).await?,
                &env,
            )
            .await,
            env.log(),
        )
    } else {
        result_to_json_response(result.map_err(|e| e.into()), env.log())
    }
}

pub async fn baking_rights(
    req: Request<Body>,
    params: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id_param = required_param!(params, "chain_id")?;
    let chain_id = parse_chain_id(chain_id_param, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    let max_priority = query.get_str("max_priority");
    let level = query.get_str("level");
    let delegate = query.get_str("delegate");
    let cycle = query.get_str("cycle");
    let has_all = query.contains_key("all");

    match services::protocol::check_and_get_baking_rights(
        &chain_id,
        &block_hash,
        level,
        delegate,
        cycle,
        max_priority,
        has_all,
        // block_metadata,
        &env,
    )
    .await
    {
        Ok(Some(rights)) => result_to_json_response(Ok(Some(rights)), env.log()),
        Ok(None) => {
            let res: Result<Option<String>, RpcServiceError> = Ok(None);
            result_to_json_response(res, env.log())
        }
        Err(RightsError::UnsupportedProtocolError { .. }) => {
            // fallback, if protocol is not supported, we trigger rpc protocol router
            let result = services::protocol::call_protocol_rpc(
                chain_id_param,
                chain_id,
                block_hash,
                create_rpc_request(req).await?,
                &env,
            )
            .await?;
            make_response_with_status_and_json_string(result.0, &result.1)
        }
        Err(RightsError::ServiceError { reason }) => {
            slog::warn!(env.log(), "Failed to execute RPC function for baking rights"; "reason" => format!("{:?}", &reason));
            handle_rpc_service_error(RpcServiceError::UnexpectedError {
                reason: format!("{}", reason),
            })
        }
    }
}

pub async fn endorsing_rights(
    req: Request<Body>,
    params: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id_param = required_param!(params, "chain_id")?;
    let chain_id = parse_chain_id(chain_id_param, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    let level = query.get_str("level");
    let cycle = query.get_str("cycle");
    let delegate = query.get_str("delegate");
    let has_all = query.contains_key("all");

    // get RPC response and unpack it from RpcResponseData enum
    match services::protocol::check_and_get_endorsing_rights(
        &chain_id,
        &block_hash,
        level,
        delegate,
        cycle,
        has_all,
        &env,
    )
    .await
    {
        Ok(Some(rights)) => result_to_json_response(Ok(Some(rights)), env.log()),
        Ok(None) => {
            let res: Result<Option<String>, RpcServiceError> = Ok(None);
            result_to_json_response(res, env.log())
        }
        Err(RightsError::UnsupportedProtocolError { .. }) => {
            // fallback, if protocol is not supported, we trigger rpc protocol router
            let result = services::protocol::call_protocol_rpc(
                chain_id_param,
                chain_id,
                block_hash,
                create_rpc_request(req).await?,
                &env,
            )
            .await?;
            make_response_with_status_and_json_string(result.0, &result.1)
        }
        Err(RightsError::ServiceError { reason }) => {
            slog::warn!(env.log(), "Failed to execute RPC function for endorsing rights"; "reason" => format!("{:?}", &reason));
            handle_rpc_service_error(RpcServiceError::UnexpectedError {
                reason: format!("{}", reason),
            })
        }
    }
}

pub async fn votes_listings(
    req: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id_param = required_param!(params, "chain_id")?;
    let chain_id = parse_chain_id(chain_id_param, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    // try to call our implementation
    match services::protocol::get_votes_listings(&chain_id, &block_hash, &env).await {
        Ok(votings) => result_to_json_response(Ok(votings), env.log()),
        Err(VotesError::UnsupportedProtocolRpc { protocol }) => {
            // if our implementation returns None, it means that the protocol does not support
            warn!(
                env.log(),
                "This rpc is not supported in protocol {}", protocol
            );
            not_found()
        }
        Err(VotesError::UnsupportedProtocolError { .. }) => {
            // fallback, if protocol is not supported in Tezedge impl, we trigger rpc protocol router
            result_to_json_response(
                services::protocol::call_protocol_rpc(
                    chain_id_param,
                    chain_id,
                    block_hash,
                    create_rpc_request(req).await?,
                    &env,
                )
                .await,
                env.log(),
            )
        }
        Err(VotesError::ServiceError { reason }) => {
            slog::warn!(env.log(), "Failed to execute RPC function  for votings"; "reason" => format!("{:?}", &reason));
            handle_rpc_service_error(RpcServiceError::UnexpectedError {
                reason: format!("{}", reason),
            })
        }
        Err(VotesError::RpcServiceError { reason }) => {
            slog::warn!(env.log(), "Failed to execute RPC function for votings"; "reason" => format!("{:?}", &reason));
            handle_rpc_service_error(reason)
        }
    }
}
pub async fn call_protocol_rpc(
    req: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id_param = required_param!(params, "chain_id")?;
    let chain_id = parse_chain_id(chain_id_param, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    let json_request = create_rpc_request(req).await?;
    let context_path = json_request.context_path.clone();
    let result = services::protocol::call_protocol_rpc(
        chain_id_param,
        chain_id,
        block_hash,
        json_request,
        &env,
    )
    .await;

    match result {
        Ok(result) => {
            let status_code = result.0;
            let body = &result.1;
            if status_code >= 400 {
                warn!(
                    env.log(),
                    "Got non-OK ({}) response from protocol-RPC service '{}', body: {:?}",
                    status_code,
                    context_path,
                    body
                );
            }
            make_response_with_status_and_json_string(status_code, body)
        }
        Err(err) => result_to_json_response::<Arc<serde_json::Value>>(Err(err), env.log()),
    }
}
