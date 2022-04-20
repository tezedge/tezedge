// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::Logger;
use warp::Filter;

use warp::filters::BoxedFilter;

use crate::monitors::resource::ResourceUtilizationStorage;
use crate::rpc::handlers::{get_measurements, MeasurementOptions};

pub fn filters(
    log: Logger,
    resource_utilization_storage: Vec<ResourceUtilizationStorage>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    // Allow cors from any origin
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET"]);

    let filters = resource_utilization_storage
        .into_iter()
        .map(|storage| get_measurements_filter(log.clone(), storage))
        .reduce(|combined, filter| combined.or(filter).unify().boxed())
        .expect("Failed to create combined filters");

    filters.with(cors)
}

pub fn get_measurements_filter(
    log: Logger,
    resource_utilization: ResourceUtilizationStorage,
) -> BoxedFilter<(impl warp::Reply,)> {
    let tag = resource_utilization.node().tag().clone();
    warp::path("resources")
        .and(warp::path(tag))
        .and(warp::get())
        .and(warp::query::<MeasurementOptions>())
        .and(with_log(log))
        .and(with_resource_utilization_storage(resource_utilization))
        .and_then(get_measurements)
        .boxed()
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
