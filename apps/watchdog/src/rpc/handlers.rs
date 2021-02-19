// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::collections::VecDeque;

use serde::Deserialize;
use slog::{info, Logger};
use warp::http::StatusCode;
use warp::reject;

use crate::monitors::resource::{
    ResourceUtilization, ResourceUtilizationStorage, MEASUREMENTS_MAX_CAPACITY,
};

#[derive(Debug, Deserialize)]
pub struct MeasurementOptions {
    limit: Option<usize>,
    offset: Option<usize>,
}

pub async fn get_measurements(
    options: MeasurementOptions,
    log: Logger,
    measurements_storage: ResourceUtilizationStorage,
) -> Result<impl warp::Reply, reject::Rejection> {
    let storage = measurements_storage.read().unwrap();

    info!(log, "Serving measurements");

    let ret: VecDeque<ResourceUtilization> = storage
        .clone()
        .into_iter()
        .take(options.limit.unwrap_or(MEASUREMENTS_MAX_CAPACITY))
        .collect();

    Ok(warp::reply::with_status(
        warp::reply::json(&ret),
        StatusCode::OK,
    ))
}
