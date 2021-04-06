// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::collections::VecDeque;

use merge::Merge;
use serde::Deserialize;
use slog::{info, Logger};
use warp::http::StatusCode;
use warp::reject;

use itertools::Itertools;

use crate::monitors::resource::{
    ResourceUtilization, ResourceUtilizationStorage, MEASUREMENTS_MAX_CAPACITY,
};

#[derive(Debug, Deserialize)]
pub struct MeasurementOptions {
    limit: Option<usize>,
    offset: Option<usize>,
    every_nth: Option<usize>,
}

pub async fn get_measurements(
    options: MeasurementOptions,
    log: Logger,
    measurements_storage: ResourceUtilizationStorage,
) -> Result<impl warp::Reply, reject::Rejection> {
    let storage = measurements_storage.read().unwrap();

    info!(log, "Serving measurements");

    let ret: VecDeque<ResourceUtilization> = if let Some(every_nth) = options.every_nth {
        storage
            .clone()
            .into_iter()
            .chunks(every_nth)
            .into_iter()
            .map(|chunk| {
                chunk
                    .fold1(|mut m1, m2| {
                        // merge strategy is set to ord::max, meaning folding the ResourceUtilization struxt efectivelly gets 
                        // the maximum possible value for each struct field
                        m1.merge(m2);
                        m1
                    })
                    .unwrap()
            })
            .take(options.limit.unwrap_or(MEASUREMENTS_MAX_CAPACITY))
            .collect()
    } else {
        storage
            .clone()
            .into_iter()
            .take(options.limit.unwrap_or(MEASUREMENTS_MAX_CAPACITY))
            .collect()
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&ret),
        StatusCode::OK,
    ))
}
