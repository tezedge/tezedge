// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::Logger;

use redux_rs::TimeService;

use crate::{key::CryptoService, rpc_client::RpcClient, timer::Timer};

pub struct ServiceDefault {
    pub logger: Logger,
    pub client: RpcClient,
    pub crypto: CryptoService,
    pub timer: Timer,
}

impl TimeService for ServiceDefault {}
