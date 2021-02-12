// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use slog::{Logger};
use tokio::task::JoinHandle;

use crate::monitors::resource::ResourceUtilizationStorage;

pub mod filters;
pub mod handlers;

pub fn spawn_rpc_server(rpc_port: u16, log: Logger, ocaml_resource_utilization: ResourceUtilizationStorage, tezedge_resource_utilization: ResourceUtilizationStorage) -> JoinHandle<()> {
    tokio::spawn(async move {
        let api = filters::filters(log.clone(), ocaml_resource_utilization.clone(), tezedge_resource_utilization.clone());

        warp::serve(api).run(([0, 0, 0, 0], rpc_port)).await;
    })
}