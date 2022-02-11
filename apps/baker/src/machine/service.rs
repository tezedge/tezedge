// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::Logger;

use redux_rs::TimeService;

use crate::{key::CryptoService, rpc_client::RpcClient};

pub struct ServiceDefault {
    pub logger: Logger,
    pub client: RpcClient,
    pub crypto: CryptoService,
}

impl TimeService for ServiceDefault {}
