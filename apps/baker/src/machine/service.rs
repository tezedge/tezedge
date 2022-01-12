// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::Logger;

use redux_rs::TimeService;

use crate::TezosClient;

pub struct ServiceDefault {
    pub log: Logger,
    pub client: TezosClient,
}

impl TimeService for ServiceDefault {}
