use crate::server::{Params, Query, RpcServiceEnvironment};
use crate::{make_raw_response, ServiceResult};
use hyper::{Body, Request};
use std::sync::Arc;

// Includes open api specification with binary
static OPEN_API_JSON_FILE: &[u8] = include_bytes!("../../openapi/tezedge-openapi.json");

pub async fn get_spec_file(
    _: Request<Body>,
    _: Params,
    _: Query,
    _: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    make_raw_response(OPEN_API_JSON_FILE)
}
