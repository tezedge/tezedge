// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_protocol_ipc_client::ProtocolRunnerConfiguration;

pub trait ProtocolNewService {
    fn spawn_process(&mut self, name: &str);
}

/// The service relying on mio and redux
pub struct ProtocolNewServiceDefault {
    config: ProtocolRunnerConfiguration,
}

impl ProtocolNewServiceDefault {
    pub fn new(config: ProtocolRunnerConfiguration) -> Self {
        ProtocolNewServiceDefault { config }
    }
}

impl ProtocolNewService for ProtocolNewServiceDefault {
    fn spawn_process(&mut self, name: &str) {
        // TODO(vlad)
        let _ = (name, &self.config);
    }
}
