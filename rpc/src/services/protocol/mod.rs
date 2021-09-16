// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module exposes protocol rpc services.
//!
//! Rule 1:
//!     - if service has different implementation and uses different structs for various protocols, it have to be placed and implemented in correct subpackage proto_XYZ
//!     - and here will be just redirector to correct subpackage by protocol_hash
//! Rule 2:
//!     - if service has the same implementation for various protocol, can be place directly here
//!     - if in new version of protocol is changed behavior, we have to splitted it here aslo by protocol_hash

use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

use anyhow::{bail, format_err, Error};
use thiserror::Error;

use crypto::hash::{BlockHash, ChainId, FromBytesError, ProtocolHash};
use storage::{BlockHeaderWithHash, BlockMetaStorage, BlockMetaStorageReader, BlockStorage, BlockStorageReader, ConstantsStorage, CycleMetaStorage};
use tezos_api::ffi::{HelpersPreapplyBlockRequest, ProtocolRpcRequest, RpcMethod, RpcRequest};
use tezos_context::context_key_owned;
use tezos_messages::base::rpc_support::RpcJsonMap;
use tezos_messages::base::signature_public_key_hash::ConversionError;
use tezos_messages::protocol::{SupportedProtocol, UnsupportedProtocolError};

use crate::helpers::RpcServiceError;
use crate::server::RpcServiceEnvironment;
use crate::services::base_services::{get_context_hash, get_raw_block_header_with_hash};
use tezos_wrapper::TezedgeContextClientError;

mod proto_001;
mod proto_002;
mod proto_003;
mod proto_004;
mod proto_005_2;
mod proto_006;
mod proto_007;
mod proto_008;
mod proto_008_2;
mod proto_009;
mod proto_010;

use cached::proc_macro::cached;
use cached::TimedSizedCache;

#[derive(Debug, Error)]
pub enum RightsError {
    #[error("Rights error, reason: {reason}")]
    ServiceError { reason: Error },
    #[error("Unsupported protocol {protocol}")]
    UnsupportedProtocolError { protocol: String },
}

impl From<ContextParamsError> for RightsError {
    fn from(error: ContextParamsError) -> Self {
        match error {
            ContextParamsError::UnsupportedProtocolError { protocol } => {
                RightsError::UnsupportedProtocolError { protocol }
            }
            _ => RightsError::ServiceError {
                reason: error.into(),
            },
        }
    }
}

impl From<anyhow::Error> for RightsError {
    fn from(error: anyhow::Error) -> Self {
        RightsError::ServiceError { reason: error }
    }
}

/// Return generated baking rights.
///
/// # Arguments
///
/// * `chain_id` - Url path parameter 'chain_id'.
/// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
/// * `level` - Url query parameter 'level'.
/// * `delegate` - Url query parameter 'delegate'.
/// * `cycle` - Url query parameter 'cycle'.
/// * `max_priority` - Url query parameter 'max_priority'.
/// * `has_all` - Url query parameter 'all'.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
///
/// Prepare all data to generate baking rights and then use Tezos PRNG to generate them.
#[cached(
    name = "BAKING_RIGHTS_CACHE",
    type = "TimedSizedCache<(BlockHash, Option<String>, Option<String>, Option<String>, Option<String>, bool), Option<Vec<RpcJsonMap>>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(block_hash.clone(), level.map(|v| v.to_string()), delegate.map(|v| v.to_string()), cycle.map(|v| v.to_string()), max_priority.map(|v| v.to_string()), has_all)}",
    result = true,
)]
pub(crate) async fn check_and_get_baking_rights(
    block_hash: &BlockHash,
    level: Option<&str>,
    delegate: Option<&str>,
    cycle: Option<&str>,
    max_priority: Option<&str>,
    has_all: bool,
    env: &RpcServiceEnvironment,
) -> Result<Option<Vec<RpcJsonMap>>, RightsError> {
    // get protocol and constants

    let context_proto_params = get_context_protocol_params(block_hash, env)?;
    let cycle_meta_storage = CycleMetaStorage::new(env.persistent_storage());

    // split impl by protocol
    match context_proto_params.protocol_hash {
        SupportedProtocol::Proto001 => proto_001::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto002 => proto_002::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto003 => proto_003::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto004 => proto_004::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto005 => panic!("not yet implemented!"),
        SupportedProtocol::Proto005_2 => proto_005_2::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto006 => proto_006::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto007 => proto_007::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto008 => proto_008::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto008_2 => proto_008_2::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto009 => proto_009::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto010 => proto_010::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            &cycle_meta_storage,
            env,
        )
        .await
        .map_err(RightsError::from),
    }
}

/// Return generated endorsing rights.
///
/// # Arguments
///
/// * `chain_id` - Url path parameter 'chain_id'.
/// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
/// * `level` - Url query parameter 'level'.
/// * `delegate` - Url query parameter 'delegate'.
/// * `cycle` - Url query parameter 'cycle'.
/// * `has_all` - Url query parameter 'all'.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
///
/// Prepare all data to generate endorsing rights and then use Tezos PRNG to generate them.
#[cached(
    name = "ENDORSING_RIGHTS_CACHE",
    type = "TimedSizedCache<(BlockHash, Option<String>, Option<String>, Option<String>, bool), Option<Vec<RpcJsonMap>>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(block_hash.clone(), level.map(|v| v.to_string()), delegate.map(|v| v.to_string()), cycle.map(|v| v.to_string()), has_all)}",
    result = true,
)]
pub(crate) async fn check_and_get_endorsing_rights(
    block_hash: &BlockHash,
    level: Option<&str>,
    delegate: Option<&str>,
    cycle: Option<&str>,
    has_all: bool,
    env: &RpcServiceEnvironment,
) -> Result<Option<Vec<RpcJsonMap>>, RightsError> {
    // get protocol and constants
    let context_proto_params = get_context_protocol_params(block_hash, env)?;
    let cycle_meta_storage = CycleMetaStorage::new(env.persistent_storage());

    // split impl by protocol
    match context_proto_params.protocol_hash {
        SupportedProtocol::Proto001 => proto_001::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto002 => proto_002::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto003 => proto_003::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto004 => proto_004::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto005 => panic!("not yet implemented!"),
        SupportedProtocol::Proto005_2 => {
            proto_005_2::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                has_all,
                &cycle_meta_storage,
            )
            .await
            .map_err(RightsError::from)
        }
        SupportedProtocol::Proto006 => proto_006::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto007 => proto_007::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto008 => proto_008::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto008_2 => {
            proto_008_2::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                has_all,
                &cycle_meta_storage,
            )
            .await
            .map_err(RightsError::from)
        }
        SupportedProtocol::Proto009 => proto_009::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            &cycle_meta_storage,
        )
        .await
        .map_err(RightsError::from),
        SupportedProtocol::Proto010 => proto_010::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            &cycle_meta_storage,
            env,
        )
        .await
        .map_err(RightsError::from),
    }
}

#[derive(Debug, Error)]
pub enum VotesError {
    #[error("Rpc service error, reason: {reason}")]
    RpcServiceError { reason: RpcServiceError },
    #[error("Votes error, reason: {reason}")]
    ServiceError { reason: Error },
    #[error("Unsupported protocol {protocol}")]
    UnsupportedProtocolError { protocol: String },
    #[error("This rpc is not suported in this protocol {protocol}")]
    UnsupportedProtocolRpc { protocol: String },
}

impl From<anyhow::Error> for VotesError {
    fn from(error: anyhow::Error) -> Self {
        VotesError::ServiceError { reason: error }
    }
}

impl From<TezedgeContextClientError> for VotesError {
    fn from(error: TezedgeContextClientError) -> Self {
        VotesError::ServiceError {
            reason: error.into(),
        }
    }
}

impl From<ConversionError> for VotesError {
    fn from(error: ConversionError) -> Self {
        VotesError::ServiceError {
            reason: error.into(),
        }
    }
}

impl From<UnsupportedProtocolError> for VotesError {
    fn from(error: UnsupportedProtocolError) -> Self {
        VotesError::UnsupportedProtocolError {
            protocol: error.protocol,
        }
    }
}

impl From<serde_json::Error> for VotesError {
    fn from(error: serde_json::Error) -> Self {
        VotesError::ServiceError {
            reason: error.into(),
        }
    }
}

impl From<FromBytesError> for VotesError {
    fn from(error: FromBytesError) -> Self {
        VotesError::ServiceError {
            reason: error.into(),
        }
    }
}

impl From<RpcServiceError> for VotesError {
    fn from(reason: RpcServiceError) -> Self {
        VotesError::RpcServiceError { reason }
    }
}

pub(crate) fn get_votes_listings(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<Option<serde_json::Value>, VotesError> {
    // TODO - TE-261: this will not work with Irmin right now, we should check that or
    // try to reimplement the missing parts on top of Irmin too.
    // if only_irmin {
    //     return Err(ContextParamsError::UnsupportedProtocolError {
    //         protocol: "only-supported-with-tezedge-context".to_string(),
    //     });
    // }
    let context_hash = get_context_hash(chain_id, block_hash, env)?;

    // get protocol version
    let protocol_hash = if let Some(protocol_hash) = env
        .tezedge_context()
        .get_key_from_history(&context_hash, context_key_owned!("protocol"))?
    {
        ProtocolHash::try_from(protocol_hash)?
    } else {
        return Err(VotesError::ServiceError {
            reason: format_err!(
                "No protocol found in context for block_hash: {}",
                block_hash.to_base58_check()
            ),
        });
    };

    // check if we support impl for this protocol
    let supported_protocol = SupportedProtocol::try_from(protocol_hash)?;
    match supported_protocol {
        SupportedProtocol::Proto001 | SupportedProtocol::Proto002 => {
            Err(VotesError::UnsupportedProtocolRpc {
                protocol: supported_protocol.protocol_hash(),
            })
        }
        SupportedProtocol::Proto003 => {
            proto_003::votes_services::get_votes_listings(env, &context_hash)
        }
        SupportedProtocol::Proto004 => {
            proto_004::votes_services::get_votes_listings(env, &context_hash)
        }
        SupportedProtocol::Proto005 => Err(VotesError::UnsupportedProtocolError {
            protocol: supported_protocol.protocol_hash(),
        }),
        SupportedProtocol::Proto005_2 => {
            proto_005_2::votes_services::get_votes_listings(env, &context_hash)
        }
        SupportedProtocol::Proto006 => {
            proto_006::votes_services::get_votes_listings(env, &context_hash)
        }
        SupportedProtocol::Proto007 => {
            proto_007::votes_services::get_votes_listings(env, &context_hash)
        }
        SupportedProtocol::Proto008 => {
            proto_008::votes_service::get_votes_listings(env, &context_hash)
        }
        SupportedProtocol::Proto008_2 => {
            proto_008_2::votes_service::get_votes_listings(env, &context_hash)
        }
        SupportedProtocol::Proto009 => {
            proto_009::votes_service::get_votes_listings(env, &context_hash)
        }
        SupportedProtocol::Proto010 => {
            proto_010::votes_service::get_votes_listings(env, &context_hash)
        }
    }
}

/// Get protocol context constants from context list
/// (just for RPC render use-case, do not use in processing or algorithms)
///
/// # Arguments
///
/// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
/// * `opt_level` - Optionaly input block level from block_id if is already known to prevent double code execution.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
pub(crate) fn get_context_constants_just_for_rpc(
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<Option<String>, ContextParamsError> {
    // TODO: just get constants from the constants storage
    let context_proto_params = get_context_protocol_params(block_hash, env)?;
    // TODO: TEST THIS
    Ok(Some(context_proto_params.constants_data))
}

// We want error responses to be errors in `call_protocol_rpc_with_cache`
// so that they don't get cached, we do so with this enum to separate
// error responses from ok responses.
pub enum RpcCallError {
    Failure(anyhow::Error),
    NoDataFound(String),
    ErrorResponse(Arc<(u16, String)>),
}

impl<F> From<F> for RpcCallError
where
    F: Into<anyhow::Error>,
{
    fn from(error: F) -> Self {
        Self::Failure(error.into())
    }
}

// NB: handles multiple paths for RPC calls
pub const TIMED_SIZED_CACHE_SIZE: usize = 500;
pub const TIMED_SIZED_CACHE_TTL_IN_SECS: u64 = 60;
#[cached(
    name = "CALL_PROTOCOL_RPC_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash, String), Arc<(u16, String)>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone(), rpc_request.ffi_rpc_router_cache_key())}",
    result = true
)]
pub(crate) fn call_protocol_rpc_with_cache(
    chain_param: &str,
    chain_id: ChainId,
    block_hash: BlockHash,
    rpc_request: RpcRequest,
    env: &RpcServiceEnvironment,
) -> Result<Arc<(u16, String)>, RpcCallError> {
    let request = create_protocol_rpc_request(chain_param, chain_id, block_hash, rpc_request, env)?;

    let controller = env.tezos_readonly_api().pool.get()?;
    let result = controller.api.call_protocol_rpc(request);

    // The protocol runner is considerable to be in an broken state
    // if we get a timeout in a second call after which we got a timeout
    // already. In that case we shut that protocol runner down.
    let broken_protocol_runner = match &result {
        Ok(_) => false,
        Err(error) => error.is_ipc_timeout_chain(),
    };

    if broken_protocol_runner {
        controller.set_release_on_return_to_pool();
    }

    // TODO: retry on other errors?

    let response = result?;
    let status_code = response.status_code();
    let body = response.body_json_string_or_empty();

    // We don't want to cache these
    if status_code >= 400 {
        Err(RpcCallError::ErrorResponse(Arc::new((status_code, body))))
    } else {
        Ok(Arc::new((status_code, body)))
    }
}

pub(crate) fn call_protocol_rpc(
    chain_param: &str,
    chain_id: ChainId,
    block_hash: BlockHash,
    rpc_request: RpcRequest,
    env: &RpcServiceEnvironment,
) -> Result<Arc<(u16, String)>, RpcServiceError> {
    match rpc_request.meth {
        RpcMethod::GET => {
            //uses cache if the request is GET request
            match call_protocol_rpc_with_cache(chain_param, chain_id, block_hash, rpc_request, env)
            {
                Ok(response) => Ok(response),
                Err(RpcCallError::ErrorResponse(response)) => Ok(response),
                Err(RpcCallError::Failure(failure)) => Err(RpcServiceError::UnexpectedError {
                    reason: format!("{}", failure),
                }),
                Err(RpcCallError::NoDataFound(msg)) => {
                    Err(RpcServiceError::NoDataFoundError { reason: msg })
                }
            }
        }
        _ => {
            let request = match create_protocol_rpc_request(
                chain_param,
                chain_id,
                block_hash,
                rpc_request,
                env,
            ) {
                Ok(response) => response,
                Err(RpcCallError::ErrorResponse(failure)) => {
                    return Err(RpcServiceError::UnexpectedError {
                        reason: format!("{:?}", failure),
                    })
                }
                Err(RpcCallError::Failure(failure)) => {
                    return Err(RpcServiceError::UnexpectedError {
                        reason: format!("{}", failure),
                    })
                }
                Err(RpcCallError::NoDataFound(msg)) => {
                    return Err(RpcServiceError::NoDataFoundError { reason: msg })
                }
            };

            // TODO: retry?
            let response = env
                .tezos_readonly_api()
                .pool
                .get()?
                .api
                .call_protocol_rpc(request)
                .map_err(|e| RpcServiceError::UnexpectedError {
                    reason: format!("Failed to call protocol rpc, reason: {}", e),
                })?;

            Ok(Arc::new((
                response.status_code(),
                response.body_json_string_or_empty(),
            )))
        }
    }
}

pub(crate) fn preapply_operations(
    chain_param: &str,
    chain_id: ChainId,
    block_hash: BlockHash,
    rpc_request: RpcRequest,
    env: &RpcServiceEnvironment,
) -> Result<serde_json::value::Value, RpcServiceError> {
    let request =
        match create_protocol_rpc_request(chain_param, chain_id, block_hash, rpc_request, env) {
            Ok(response) => response,
            Err(RpcCallError::ErrorResponse(failure)) => {
                return Err(RpcServiceError::UnexpectedError {
                    reason: format!("{:?}", failure),
                })
            }
            Err(RpcCallError::Failure(failure)) => {
                return Err(RpcServiceError::UnexpectedError {
                    reason: format!("{}", failure),
                })
            }
            Err(RpcCallError::NoDataFound(msg)) => {
                return Err(RpcServiceError::NoDataFoundError { reason: msg })
            }
        };

    // TODO: retry?
    let response = env
        .tezos_readonly_api()
        .pool
        .get()?
        .api
        .helpers_preapply_operations(request)
        .map_err(|e| RpcServiceError::UnexpectedError {
            reason: format!("Failed to call helpers_preapply_operations, reason: {}", e),
        })?;

    serde_json::from_str(&response.body).map_err(|e| e.into())
}

pub(crate) fn preapply_block(
    chain_param: &str,
    chain_id: ChainId,
    block_hash: BlockHash,
    rpc_request: RpcRequest,
    env: &RpcServiceEnvironment,
) -> Result<serde_json::value::Value, RpcServiceError> {
    let block_storage = BlockStorage::new(env.persistent_storage());
    let block_meta_storage = BlockMetaStorage::new(env.persistent_storage());
    let (block_header, (predecessor_block_metadata_hash, predecessor_ops_metadata_hash)) =
        match block_storage.get(&block_hash)? {
            Some(block_header) => match block_meta_storage.get_additional_data(&block_hash)? {
                Some(block_header_additional_data) => {
                    (block_header, block_header_additional_data.into())
                }
                None => {
                    return Err(RpcServiceError::NoDataFoundError {
                        reason: format!(
                            "No block additioanl data found for hash: {}",
                            block_hash.to_base58_check()
                        ),
                    })
                }
            },
            None => {
                return Err(RpcServiceError::NoDataFoundError {
                    reason: format!(
                        "No block header found for hash: {}",
                        block_hash.to_base58_check()
                    ),
                })
            }
        };

    // create request to ffi
    let request = HelpersPreapplyBlockRequest {
        protocol_rpc_request: ProtocolRpcRequest {
            chain_arg: chain_param.to_string(),
            block_header: block_header.header.as_ref().clone(),
            request: rpc_request,
            chain_id,
        },
        predecessor_block_metadata_hash,
        predecessor_ops_metadata_hash,
    };

    // TODO: retry?
    let response = env
        .tezos_readonly_api()
        .pool
        .get()?
        .api
        .helpers_preapply_block(request)
        .map_err(|e| RpcServiceError::UnexpectedError {
            reason: format!("Failed to call helpers_preapply_block, reason: {}", e),
        })?;

    serde_json::from_str(&response.body).map_err(|e| e.into())
}

fn create_protocol_rpc_request(
    chain_param: &str,
    chain_id: ChainId,
    block_hash: BlockHash,
    rpc_request: RpcRequest,
    env: &RpcServiceEnvironment,
) -> Result<ProtocolRpcRequest, RpcCallError> {
    // get block header
    let block_header =
        get_raw_block_header_with_hash(&chain_id, &block_hash, env.persistent_storage())
            .map(|block_header| block_header.header.as_ref().clone())
            .map_err(|e| match e {
                RpcServiceError::NoDataFoundError { reason } => RpcCallError::NoDataFound(reason),
                rpce => RpcCallError::Failure(rpce.into()),
            })?;

    // create request to ffi
    Ok(ProtocolRpcRequest {
        chain_arg: chain_param.to_string(),
        block_header,
        request: rpc_request,
        chain_id,
    })
}

pub(crate) struct ContextProtocolParam {
    pub protocol_hash: SupportedProtocol,
    pub constants_data: String,
    pub block_header: BlockHeaderWithHash,
    // pub block_cycle: i32,
    // pub block_cycle_position: i32,
}

#[derive(Debug, Error)]
#[allow(clippy::enum_variant_names)]
pub enum ContextParamsError {
    #[error("Storage error occurred, reason: {reason}")]
    StorageError { reason: storage::StorageError },
    #[error("Context error occurred, reason: {reason}")]
    ContextError { reason: TezedgeContextClientError },
    #[error("Context constants, reason: {reason}")]
    ContextConstantsDecodeError {
        reason: tezos_messages::protocol::ContextConstantsDecodeError,
    },
    #[error("Unsupported protocol {protocol}")]
    UnsupportedProtocolError { protocol: String },
    #[error("Hash error {error}")]
    HashError { error: FromBytesError },
}

impl From<storage::StorageError> for ContextParamsError {
    fn from(error: storage::StorageError) -> Self {
        ContextParamsError::StorageError { reason: error }
    }
}

impl From<TezedgeContextClientError> for ContextParamsError {
    fn from(error: TezedgeContextClientError) -> Self {
        ContextParamsError::ContextError { reason: error }
    }
}

impl From<UnsupportedProtocolError> for ContextParamsError {
    fn from(error: UnsupportedProtocolError) -> Self {
        ContextParamsError::UnsupportedProtocolError {
            protocol: error.protocol,
        }
    }
}

impl From<tezos_messages::protocol::ContextConstantsDecodeError> for ContextParamsError {
    fn from(error: tezos_messages::protocol::ContextConstantsDecodeError) -> Self {
        ContextParamsError::ContextConstantsDecodeError { reason: error }
    }
}

impl From<FromBytesError> for ContextParamsError {
    fn from(error: FromBytesError) -> ContextParamsError {
        ContextParamsError::HashError { error }
    }
}

#[allow(clippy::from_over_into)]
impl Into<RpcServiceError> for ContextParamsError {
    fn into(self) -> RpcServiceError {
        match self {
            ContextParamsError::StorageError { reason } => {
                RpcServiceError::StorageError { error: reason }
            }
            e => RpcServiceError::UnexpectedError {
                reason: format!("{}", e),
            },
        }
    }
}

/// Get protocol and context constants as bytes from context list for desired block or level
///
/// # Arguments
///
/// * `block_hash` - [BlockHash]
/// * `opt_level` - Optionaly input block level from block_id if is already known to prevent double code execution.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
pub(crate) fn get_context_protocol_params(
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<ContextProtocolParam, ContextParamsError> {
    // TODO - TE-261: this will not work with Irmin right now, we should check that or
    // try to reimplement the missing parts on top of Irmin too.
    // if only_irmin {
    //     return Err(ContextParamsError::UnsupportedProtocolError {
    //         protocol: "only-supported-with-tezedge-context".to_string(),
    //     });
    // }

    // get block header
    let block_header = match BlockStorage::new(env.persistent_storage()).get(block_hash)? {
        Some(block) => block,
        None => {
            return Err(storage::StorageError::MissingKey {
                when: "get_context_protocol_params".into(),
            }
            .into())
        }
    };

    slog::crit!(env.log(), "Getting block meta from storage");

    let protocol_hash =
        match BlockMetaStorage::new(env.persistent_storage()).get_additional_data(block_hash)? {
            Some(block) => block.next_protocol_hash,
            None => {
                return Err(storage::StorageError::MissingKey {
                    when: "get_context_protocol_params".into(),
                }
                .into())
            }
        };

    slog::crit!(
        env.log(),
        "Getting constants from storage - proto: {}",
        protocol_hash.to_base58_check()
    );

    let constants = match ConstantsStorage::new(env.persistent_storage()).get(&protocol_hash)? {
        Some(constants) => constants,
        None => {
            return Err(storage::StorageError::MissingKey {
                when: "get_context_protocol_params".into(),
            }
            .into())
        }
    };

    slog::crit!(env.log(), "CONSTANTS: {}", constants);

    Ok(ContextProtocolParam {
        protocol_hash: protocol_hash.try_into()?,
        constants_data: constants,
        block_header,
    })
}

pub fn get_blocks_per_cycle(protocol_hash: &ProtocolHash, serialized_constants: &str) -> Result<i32, anyhow::Error> {
    let supported_protocol = SupportedProtocol::try_from(protocol_hash)?;

    match supported_protocol {
        SupportedProtocol::Proto001 => {
            Ok(serde_json::from_str::<proto_001::ProtocolConstants>(serialized_constants)?.blocks_per_cycle())
        }
        SupportedProtocol::Proto002 => {
            Ok(serde_json::from_str::<proto_002::ProtocolConstants>(serialized_constants)?.blocks_per_cycle())
        }
        SupportedProtocol::Proto003 => {
            Ok(serde_json::from_str::<proto_003::ProtocolConstants>(serialized_constants)?.blocks_per_cycle())
        }
        SupportedProtocol::Proto004 => {
            Ok(serde_json::from_str::<proto_004::ProtocolConstants>(serialized_constants)?.blocks_per_cycle())
        }
        SupportedProtocol::Proto005 => bail!("Not implemented"),
        SupportedProtocol::Proto005_2 => {
            Ok(serde_json::from_str::<proto_005_2::ProtocolConstants>(serialized_constants)?.blocks_per_cycle())
        }
        SupportedProtocol::Proto006 => {
            Ok(serde_json::from_str::<proto_006::ProtocolConstants>(serialized_constants)?.blocks_per_cycle())
        }
        SupportedProtocol::Proto007 => {
            Ok(serde_json::from_str::<proto_007::ProtocolConstants>(serialized_constants)?.blocks_per_cycle())
        }
        SupportedProtocol::Proto008 => {
            Ok(serde_json::from_str::<proto_008::ProtocolConstants>(serialized_constants)?.blocks_per_cycle())
        }
        SupportedProtocol::Proto008_2 => {
            Ok(serde_json::from_str::<proto_008_2::ProtocolConstants>(serialized_constants)?.blocks_per_cycle())
        }
        SupportedProtocol::Proto009 => {
            Ok(serde_json::from_str::<proto_009::ProtocolConstants>(serialized_constants)?.blocks_per_cycle())
        }
        SupportedProtocol::Proto010 => {
            Ok(serde_json::from_str::<proto_010::ProtocolConstants>(serialized_constants)?.blocks_per_cycle())
        }
    }
}

pub mod string_to_int {
    use serde::{de::Error as _, Deserialize, Deserializer};

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: std::str::FromStr,
        <T as std::str::FromStr>::Err: std::fmt::Display,
    {
        String::deserialize(deserializer)?
            .parse::<T>()
            .map_err(|e| D::Error::custom(format!("{}", e)))
    }
}

pub mod vec_string_to_int {
    use serde::{de::Error as _, Deserialize, Deserializer};

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: std::str::FromStr,
        <T as std::str::FromStr>::Err: std::fmt::Display,
    {
        Vec::deserialize(deserializer)?.into_iter().map(|v: String| v.parse::<T>().map_err(|e| D::Error::custom(format!("{}", e)))).collect()
    }
}