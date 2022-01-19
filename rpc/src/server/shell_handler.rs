// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::sync::Arc;

use hyper::body::Buf;
use hyper::{Body, Method, Request};

use tokio_stream::{wrappers::UnboundedReceiverStream, StreamExt};

use crypto::hash::ProtocolHash;
use shell_automaton::service::rpc_service::RpcRequestStream;

use crate::helpers::{
    create_rpc_request, parse_async, parse_block_hash, parse_chain_id, RpcServiceError,
    MAIN_CHAIN_ID,
};
use crate::server::{HResult, HasSingleValue, Params, Query, RpcServiceEnvironment};
use crate::services::{base_services, stream_services};
use crate::{
    empty, encoding::base_types::*, error, helpers, make_json_response, make_json_stream_response,
    not_found, parse_block_hash_or_fail, required_param, result_to_empty_json_response,
    result_to_json_response, services, ServiceResult,
};

pub async fn bootstrapped(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let stream = env
        .shell_automaton_sender()
        .request_stream(RpcRequestStream::Bootstrapped)
        .await
        .ok()
        .expect("state machine should be correct");
    let stream =
        UnboundedReceiverStream::new(stream).map(|v| serde_json::to_string(&v).map_err(From::from));
    make_json_stream_response(stream)
}

pub async fn commit_hash(
    _: Request<Body>,
    _: Params,
    _: Query,
    _: Arc<RpcServiceEnvironment>,
) -> HResult {
    let resp = &UniString::from(env!("GIT_HASH"));
    make_json_response(&resp)
}

pub async fn active_chains(
    _: Request<Body>,
    _: Params,
    _: Query,
    _: Arc<RpcServiceEnvironment>,
) -> HResult {
    empty()
}

pub async fn protocols(
    _: Request<Body>,
    _: Params,
    _: Query,
    _: Arc<RpcServiceEnvironment>,
) -> HResult {
    empty()
}

pub async fn valid_blocks(
    _: Request<Body>,
    _: Params,
    _: Query,
    _: Arc<RpcServiceEnvironment>,
) -> HResult {
    empty()
}

pub async fn head_chain(
    _: Request<Body>,
    params: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let _chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let protocol = if let Some(protocol) = query.get_str("next_protocol") {
        ProtocolHash::from_base58_check(protocol).ok()
    } else {
        None
    };

    make_json_stream_response(stream_services::HeadMonitorStream::new(
        env.state.clone(),
        protocol,
        &env.persistent_storage,
        env.log.clone(),
    ))
}

pub async fn mempool_monitor_operations(
    _: Request<Body>,
    params: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    // TODO: use it
    let _chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;

    let applied = query.get_str("applied");
    let branch_refused = query.get_str("branch_refused");
    let branch_delayed = query.get_str("branch_delayed");
    let refused = query.get_str("refused");

    let stream = env
        .shell_automaton_sender()
        .request_stream(RpcRequestStream::GetOperations {
            applied: applied == Some("yes"),
            branch_refused: branch_refused == Some("yes"),
            branch_delayed: branch_delayed == Some("yes"),
            refused: refused == Some("yes"),
        })
        .await
        .ok()
        .expect("state machine should be correct");
    let stream =
        UnboundedReceiverStream::new(stream).map(|v| serde_json::to_string(&v).map_err(From::from));
    make_json_stream_response(stream)
}

pub async fn blocks(
    _: Request<Body>,
    params: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let length = query.get_str("length").unwrap_or("0");
    let head_param = query
        .get("head")
        .cloned()
        .unwrap_or(vec!["head".to_string()]);
    let head_param = if head_param.is_empty() {
        vec!["head".to_string()]
    } else {
        head_param
    };
    let mut hashes = Vec::with_capacity(head_param.len());
    for head_param in head_param {
        hashes.push(parse_block_hash_or_fail!(&chain_id, &head_param, &env));
    }
    let min_date = query.get_str("min_date").unwrap_or("0");

    // TODO: This can be implemented in a more optimised and cleaner way
    // Note: Need to investigate the "more heads per level" variant

    let block_hashes = base_services::get_blocks(
        chain_id,
        hashes,
        length.parse::<usize>()?,
        min_date.parse::<i64>()?,
        env.persistent_storage(),
    );

    result_to_json_response(block_hashes, env.log())
}

pub async fn chains_block_id(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    result_to_json_response(
        base_services::get_block(&chain_id, &block_hash, &env).await,
        env.log(),
    )
}

pub async fn chains_block_id_header(
    _req: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    result_to_json_response(
        base_services::get_block_header(chain_id, block_hash, env.persistent_storage()).await,
        env.log(),
    )
}

pub async fn chains_block_id_header_shell(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    let result = base_services::get_block_shell_header_or_fail(
        &chain_id,
        block_hash,
        env.persistent_storage(),
    );

    match result {
        Ok(result) => result_to_json_response(Ok(result), env.log()),
        Err(e) => match e {
            RpcServiceError::NoDataFoundError { .. } => not_found(),
            e => error(e.into()),
        },
    }
}

pub async fn chains_block_id_header_protocol_data_raw(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    let block_header = base_services::get_block_raw_header_or_fail(
        &chain_id,
        block_hash,
        env.persistent_storage(),
    );

    result_to_json_response(
        block_header.map(|header| hex::encode(header.header.protocol_data())),
        env.log(),
    )
}

pub async fn chains_block_id_metadata(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    result_to_json_response(
        base_services::get_block_metadata(&chain_id, &block_hash, &env).await,
        env.log(),
    )
}

pub async fn context_raw_bytes(
    _: Request<Body>,
    params: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);
    let prefix = params.get_str("any").map(|s| s.to_owned());
    let depth = query.get_usize("depth");

    result_to_json_response(
        base_services::get_context_raw_bytes(&chain_id, &block_hash, prefix, depth, &env).await,
        env.log(),
    )
}

pub async fn mempool_pending_operations(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let log = env.log.clone();
    let pending_operations =
        services::mempool_services::get_pending_operations(&chain_id, env).await;
    result_to_json_response(pending_operations, &log)
}

pub async fn inject_operation(
    req: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let operation_data_raw = hyper::body::aggregate(req).await?;
    let operation_data: String = serde_json::from_reader(&mut operation_data_raw.reader())?;

    let chain_id_query = query.get_str("chain_id").unwrap_or(MAIN_CHAIN_ID);
    let chain_id = parse_chain_id(chain_id_query, &env)?;
    let is_async = parse_async(&query, false);

    result_to_json_response(
        services::mempool_services::inject_operation(is_async, chain_id, &operation_data, &env)
            .await,
        env.log(),
    )
}

pub async fn inject_block(
    req: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let body = hyper::body::to_bytes(req.into_body()).await?;
    let body = String::from_utf8(body.to_vec())?;

    let chain_id_query = query.get_str("chain_id").unwrap_or(MAIN_CHAIN_ID);
    let chain_id = parse_chain_id(chain_id_query, &env)?;
    let is_async = parse_async(&query, false);

    result_to_json_response(
        services::mempool_services::inject_block(is_async, chain_id, &body, &env).await,
        env.log(),
    )
}

pub async fn mempool_request_operations(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    result_to_empty_json_response(
        services::mempool_services::request_operations(&env).await,
        env.log(),
    )
}

pub async fn get_block_protocols(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    result_to_json_response(
        base_services::get_block_protocols(&chain_id, &block_hash, env.persistent_storage()),
        env.log(),
    )
}

pub async fn get_block_hash(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    result_to_json_response(Ok(block_hash.to_base58_check()), env.log())
}

pub async fn get_chain_id(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;

    result_to_json_response(Ok(chain_id.to_base58_check()), env.log())
}

pub async fn get_metadata_hash(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    match base_services::get_additional_data_or_fail(
        &chain_id,
        &block_hash,
        env.persistent_storage(),
    ) {
        Ok(data) => match data.block_metadata_hash() {
            Some(hash) => result_to_json_response(Ok(hash.to_base58_check()), env.log()),
            None => not_found(),
        },
        Err(e) => match e {
            RpcServiceError::NoDataFoundError { .. } => not_found(),
            e => error(e.into()),
        },
    }
}

pub async fn get_operations_metadata_hash(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    match base_services::get_additional_data_or_fail(
        &chain_id,
        &block_hash,
        env.persistent_storage(),
    ) {
        Ok(data) => match data.ops_metadata_hash() {
            Some(hash) => result_to_json_response(Ok(hash.to_base58_check()), env.log()),
            None => not_found(),
        },
        Err(e) => match e {
            RpcServiceError::NoDataFoundError { .. } => not_found(),
            e => error(e.into()),
        },
    }
}

pub async fn get_operations_metadata_hash_operation_metadata_hashes(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    match base_services::get_additional_data_or_fail(
        &chain_id,
        &block_hash,
        env.persistent_storage(),
    ) {
        Ok(data) => match data.ops_metadata_hashes() {
            Some(hashes) => {
                let hashes = hashes
                    .iter()
                    .map(|ops| {
                        ops.iter()
                            .map(|hash| hash.to_base58_check())
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                result_to_json_response(Ok(hashes), env.log())
            }
            None => not_found(),
        },
        Err(e) => match e {
            RpcServiceError::NoDataFoundError { .. } => not_found(),
            e => error(e.into()),
        },
    }
}

pub async fn get_operations_metadata_hash_operation_metadata_hashes_by_validation_pass(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);
    let validation_pass: usize = required_param!(params, "validation_pass_index")?.parse()?;

    match base_services::get_additional_data_or_fail(
        &chain_id,
        &block_hash,
        env.persistent_storage(),
    ) {
        Ok(data) => match data.ops_metadata_hashes() {
            Some(hashes) => {
                if let Some(validation_passes) = hashes.get(validation_pass) {
                    let validation_passes = validation_passes
                        .iter()
                        .map(|hash| hash.to_base58_check())
                        .collect::<Vec<_>>();
                    result_to_json_response(Ok(validation_passes), env.log())
                } else {
                    not_found()
                }
            }
            None => not_found(),
        },
        Err(e) => match e {
            RpcServiceError::NoDataFoundError { .. } => not_found(),
            e => error(e.into()),
        },
    }
}

pub async fn get_operations_metadata_hash_operation_metadata_hashes_by_validation_pass_by_operation_index(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);
    let validation_pass: usize = required_param!(params, "validation_pass_index")?.parse()?;
    let operation_index: usize = required_param!(params, "operation_index")?.parse()?;

    match base_services::get_additional_data_or_fail(
        &chain_id,
        &block_hash,
        env.persistent_storage(),
    ) {
        Ok(data) => match data.ops_metadata_hashes() {
            Some(hashes) => {
                if let Some(validation_passes) = hashes.get(validation_pass) {
                    if let Some(validation_pass) = validation_passes.get(operation_index) {
                        result_to_json_response(Ok(validation_pass.to_base58_check()), env.log())
                    } else {
                        not_found()
                    }
                } else {
                    not_found()
                }
            }
            None => not_found(),
        },
        Err(e) => match e {
            RpcServiceError::NoDataFoundError { .. } => not_found(),
            e => error(e.into()),
        },
    }
}

pub async fn get_block_operation_hashes(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    result_to_json_response(
        base_services::get_block_operation_hashes(chain_id, &block_hash, &env).await,
        env.log(),
    )
}

pub async fn get_block_operations(
    _req: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    result_to_json_response(
        base_services::get_block_operations_metadata(chain_id, &block_hash, &env).await,
        env.log(),
    )
}

pub async fn get_block_operations_validation_pass(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    let validation_pass: usize = required_param!(params, "validation_pass_index")?.parse()?;
    let res = base_services::get_block_operations_validation_pass(
        chain_id,
        &block_hash,
        &env,
        validation_pass,
    )
    .await;
    result_to_json_response(res, env.log())
}

pub async fn get_block_operation(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    let validation_pass: usize = required_param!(params, "validation_pass_index")?.parse()?;
    let operation_order: usize = required_param!(params, "operation_index")?.parse()?;

    result_to_json_response(
        base_services::get_block_operation(
            chain_id,
            &block_hash,
            &env,
            validation_pass,
            operation_order,
        )
        .await,
        env.log(),
    )
}

pub async fn live_blocks(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    result_to_json_response(
        services::base_services::live_blocks(&chain_id, block_hash, &env),
        env.log(),
    )
}

pub async fn preapply_operations(
    req: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id_param = required_param!(params, "chain_id")?;
    let chain_id = parse_chain_id(chain_id_param, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    let rpc_request = create_rpc_request(req).await?;

    result_to_json_response(
        services::protocol::preapply_operations(
            chain_id_param,
            chain_id,
            block_hash,
            rpc_request,
            &env,
        )
        .await,
        env.log(),
    )
}

pub async fn preapply_block(
    req: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id_param = required_param!(params, "chain_id")?;
    let chain_id = parse_chain_id(chain_id_param, &env)?;
    let block_hash =
        parse_block_hash_or_fail!(&chain_id, required_param!(params, "block_id")?, &env);

    let rpc_request = create_rpc_request(req).await?;

    // launcher - we need the error from preapply
    result_to_json_response(
        services::protocol::preapply_block(chain_id_param, chain_id, block_hash, rpc_request, &env)
            .await,
        env.log(),
    )
}

// TODO: properly implement this handler

#[derive(serde::Serialize)]
struct P2pStats {
    total_sent : String,
    total_recv : String,
    current_inflow : i64,
    current_outflow : i64,
}

pub async fn network_stat(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let dummy = P2pStats {
        total_sent: "100".into(),
        total_recv: "100".into(),
        current_inflow: 10,
        current_outflow: 10,
    };

    result_to_json_response(
        Ok(dummy),
        env.log(),
    )
}

pub async fn network_connections(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let connections: Vec<()> = vec![];
    result_to_json_response(
        Ok(connections),
        env.log(),
    )
}

pub async fn node_version(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    result_to_json_response(
        Ok(base_services::get_node_version(env.network_version())),
        env.log(),
    )
}

pub async fn config_user_activated_upgrades(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    result_to_json_response(
        Ok(env
            .tezos_environment()
            .protocol_overrides
            .user_activated_upgrades_to_rpc_json()),
        env.log(),
    )
}

pub async fn config_user_activated_protocol_overrides(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    result_to_json_response(
        Ok(env
            .tezos_environment()
            .protocol_overrides
            .user_activated_protocol_overrides_to_rpc_json()),
        env.log(),
    )
}

// TODO: TE-275 - implement correctly - at least for protocol rpcs. This is a 'fake it till you make it' handler
/// Handler mockin the describe routes in ocaml to be compatible with tezoses python test framework
pub async fn describe(
    allowed_methods: Arc<HashSet<Method>>,
    req: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let path: Vec<String> = req
        .uri()
        .path()
        .split('/')
        .skip(2)
        .map(|v| v.to_string())
        .collect();

    // TODO: dyanmically get the method from a protocol call, for now, get the first element...
    // NOTE: protocol rpcs are dynamically created and called trough the protocol,
    // as a next step, we should somehow get the registreds paths method from the protocol
    let method = if !allowed_methods.is_empty() {
        // TODO: same reasoning as above
        if path.contains(&"injection".to_string()) || path.contains(&"forge".to_string()) {
            &Method::POST
        } else if path.contains(&"storage".to_string()) || path.contains(&"round".to_string()) || path.contains(&"delegates".to_string()) {
            &Method::GET
        } else {
            allowed_methods.iter().next().unwrap()
        }
    } else {
        return empty();
    };

    let service_fields = serde_json::json!({
        "meth": method.as_str(),
        "path": path,
        "description": "Handler mockin the describe routes in ocaml to be compatible with tezoses python test framework.",
        "query": [],
        "output": {
            "json_schema": {},
            "binary_schema": {
                "toplevel": {
                    "fields": [],
                },
                "fields": [],
            },
        },
        "error": {
            "json_schema": {},
            "binary_schema": {
                "toplevel": {
                    "fields": [],
                },
                "fields": [],
            },
        },
    });

    let describe_json = match *method {
        Method::GET => serde_json::json!({
            "static": {
                "get_service": service_fields
            },
        }),
        Method::POST => serde_json::json!({
            "static": {
                "post_service": service_fields
            },
        }),
        Method::PUT => serde_json::json!({
            "static": {
                "put_service": service_fields
            },
        }),
        Method::DELETE => serde_json::json!({
            "static": {
                "delete_service": service_fields
            },
        }),
        _ => unimplemented!(),
    };

    result_to_json_response(Ok(describe_json), env.log())
}

pub async fn worker_prevalidators(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    result_to_json_response(helpers::get_prevalidators(&env).await, env.log())
}
