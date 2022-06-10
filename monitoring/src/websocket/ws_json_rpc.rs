use std::{collections::HashMap, pin::Pin};

use rpc::{
    server::{parse_query_string, MethodHandler},
    RpcServiceEnvironmentRef,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Params {
    Array(Vec<Value>),
    Object(Value),
}

#[derive(Clone, Debug, Deserialize)]
pub struct PostRequestParams {
    pub body: Value,
}

pub type JsonRpcResponse = json_rpc_types::Response<serde_json::Value, ()>;
pub type JsonRpcError = json_rpc_types::Error<()>;

/// Handles the json-rpc 2.0 requests and propagates it to the RPC handlers
pub async fn handle_request(
    req: &json_rpc_types::Request<Params, String>,
    env: &RpcServiceEnvironmentRef,
) -> JsonRpcResponse {
    // TODO move this out, to the connection establishment, or even to the WS construction
    let routes = rpc::server::router::create_routes(false);
    let method = &req.method;

    let request_with_body = match reconstruct_request(req, method) {
        Ok(req) => req,
        Err(e) => return json_rpc_types::Response::error(req.jsonrpc, e, req.id.clone()),
    };

    if let Some((method_and_handler, required_parameters)) = routes.find(method) {
        let MethodHandler { handler, .. } = method_and_handler;

        let required_parameters: Vec<(String, String)> = required_parameters
            .into_iter()
            .map(|(param, value)| (param.to_string(), value.to_string()))
            .collect();
        let query_args: HashMap<String, Vec<String>> = request_with_body
            .uri()
            .query()
            .map(parse_query_string)
            .unwrap_or_else(HashMap::new);

        let handler = handler.clone();
        let fut = handler(
            request_with_body,
            required_parameters,
            query_args,
            env.clone(),
        );

        match Pin::from(fut).await {
            Ok(v) => {
                let (_, body) = v.into_parts();
                let body_bytes = match warp::hyper::body::to_bytes(body).await {
                    Ok(body_bytes) => body_bytes,
                    Err(_) => {
                        let json_error = json_rpc_types::Error::with_custom_msg(
                            json_rpc_types::ErrorCode::InternalError,
                            "Failed to get bytes",
                        );
                        return json_rpc_types::Response::error(
                            req.jsonrpc,
                            json_error,
                            req.id.clone(),
                        );
                    }
                };
                let res = match serde_json::from_slice(&body_bytes) {
                    Ok(res) => res,
                    Err(_) => {
                        let json_error = json_rpc_types::Error::with_custom_msg(
                            json_rpc_types::ErrorCode::InternalError,
                            "Failed to deserialize body",
                        );
                        return json_rpc_types::Response::error(
                            req.jsonrpc,
                            json_error,
                            req.id.clone(),
                        );
                    }
                };
                json_rpc_types::Response::result(req.jsonrpc, res, req.id.clone())
            }
            Err(_) => {
                let error =
                    json_rpc_types::Error::from_code(json_rpc_types::ErrorCode::InternalError);
                json_rpc_types::Response::error(req.jsonrpc, error, req.id.clone())
            }
        }
    } else {
        let error = json_rpc_types::Error::from_code(json_rpc_types::ErrorCode::MethodNotFound);
        json_rpc_types::Response::error(req.jsonrpc, error, req.id.clone())
    }
}

/// Reconstructs a HTTP request that is accepted by the RPC handlers
fn reconstruct_request(
    req: &json_rpc_types::Request<Params, String>,
    method: &str,
) -> Result<http::Request<warp::hyper::Body>, json_rpc_types::Error<()>> {
    // fake uri, we only need it to extract query params
    // Note: The method will contain the full path even with query arguments, this way we ensure compatibility between the HTTP RPC API and the WS API
    //       even though it slightly diverges from the json-rpc 2.0 specification
    let fake_uri_string = format!("ws://127.0.0.1{method}");
    let method_uri = fake_uri_string
        .parse::<http::Uri>()
        .map_err(|_| json_rpc_types::Error::from_code(json_rpc_types::ErrorCode::InternalError))?;

    if req.params.is_some() {
        let params = extract_object_params::<PostRequestParams>(req).map_err(|_| {
            json_rpc_types::Error::from_code(json_rpc_types::ErrorCode::InvalidParams)
        })?;

        let string_body = serde_json::to_string(&params.body).map_err(|_| {
            json_rpc_types::Error::from_code(json_rpc_types::ErrorCode::InternalError)
        })?;
        let body = warp::hyper::Body::try_from(string_body).map_err(|_| {
            json_rpc_types::Error::from_code(json_rpc_types::ErrorCode::InternalError)
        })?;

        http::request::Builder::new()
            .method("POST")
            .uri(method_uri)
            .body(body)
            .map_err(|_| json_rpc_types::Error::from_code(json_rpc_types::ErrorCode::InternalError))
    } else {
        http::request::Builder::new()
            .method("GET")
            .uri(method_uri)
            .body(warp::hyper::Body::default())
            .map_err(|_| json_rpc_types::Error::from_code(json_rpc_types::ErrorCode::InternalError))
    }
}

fn extract_object_params<T: DeserializeOwned>(
    req: &json_rpc_types::Request<Params, String>,
) -> Result<T, JsonRpcError> {
    if let Some(Params::Object(params)) = &req.params {
        serde_json::from_value(params.clone()).map_err(|e| {
            JsonRpcError::with_custom_msg_truncated(
                json_rpc_types::ErrorCode::ParseError,
                &e.to_string(),
            )
        })
    } else {
        Err(JsonRpcError::from_code(
            json_rpc_types::ErrorCode::InvalidParams,
        ))
    }
}
