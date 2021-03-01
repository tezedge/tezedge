// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use slog::Logger;
use warp::Filter;

use crate::monitors::resource::ResourceUtilizationStorage;
use crate::rpc::handlers::{get_measurements, MeasurementOptions};

pub fn filters(
    log: Logger,
    ocaml_resource_utilization_storage: ResourceUtilizationStorage,
    tezedge_resource_utilization_storage: ResourceUtilizationStorage,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    // Allow cors from any origin
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET"]);

    get_ocaml_measurements_filter(log.clone(), ocaml_resource_utilization_storage)
        .or(get_tezedge_measurements_filter(
            log,
            tezedge_resource_utilization_storage,
        ))
        .with(cors)
}

pub fn get_tezedge_measurements_filter(
    log: Logger,
    tezedge_resource_utilization: ResourceUtilizationStorage,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("resources" / "tezedge")
        .and(warp::get())
        .and(warp::query::<MeasurementOptions>())
        .and(with_log(log))
        .and(with_resource_utilization_storage(
            tezedge_resource_utilization,
        ))
        .and_then(get_measurements)
}

pub fn get_ocaml_measurements_filter(
    log: Logger,
    ocaml_resource_utilization: ResourceUtilizationStorage,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("resources" / "ocaml")
        .and(warp::get())
        .and(warp::query::<MeasurementOptions>())
        .and(with_log(log))
        .and(with_resource_utilization_storage(
            ocaml_resource_utilization,
        ))
        .and_then(get_measurements)
}

fn with_log(
    log: Logger,
) -> impl Filter<Extract = (Logger,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || log.clone())
}

fn with_resource_utilization_storage(
    storage: ResourceUtilizationStorage,
) -> impl Filter<Extract = (ResourceUtilizationStorage,), Error = std::convert::Infallible> + Clone
{
    warp::any().map(move || storage.clone())
}
