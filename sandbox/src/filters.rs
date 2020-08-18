use slog::Logger;
use warp::Filter;

use crate::handlers::{start_node_with_config, stop_node};
use crate::node_runner::{LightNodeRunnerRef};

pub fn sandbox(
    log: Logger,
    runner: LightNodeRunnerRef,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    start(log.clone(), runner.clone()).or(stop(log.clone(), runner))
}

pub fn start(
    log: Logger,
    runner: LightNodeRunnerRef,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("start")
        .and(warp::post())
        .and(json_body())
        .and(with_log(log))
        .and(with_runner(runner))
        .and_then(start_node_with_config)
}

pub fn stop(
    log: Logger,
    runner: LightNodeRunnerRef,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("stop")
        .and(warp::get())
        .and(with_log(log))
        .and(with_runner(runner))
        .and_then(stop_node)
}

fn json_body() -> impl Filter<Extract = (serde_json::Value,), Error = warp::Rejection> + Clone
{
    // When accepting a body, we want a JSON body
    // (and to reject huge payloads)...
    warp::body::content_length_limit(1024 * 16).and(warp::body::json())
}

fn with_log(
    log: Logger,
) -> impl Filter<Extract = (Logger,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || log.clone())
}

fn with_runner(
    runner: LightNodeRunnerRef,
) -> impl Filter<Extract = (LightNodeRunnerRef,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || runner.clone())
}
