// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use serde::Deserialize;

use crate::environment::{TezosEnvironmentConfiguration, TezosNetworkConfigurationError};
use std::convert::{TryFrom, TryInto};
use tezos_context_api::{GenesisChain, PatchContext, ProtocolOverrides};

#[derive(Deserialize, Debug)]
pub struct OctezConfig {
    network: OctezCustomNetwork,
}

impl OctezConfig {
    pub fn take_network(
        self,
    ) -> Result<TezosEnvironmentConfiguration, TezosNetworkConfigurationError> {
        self.network.try_into()
    }
}

#[derive(Deserialize, Debug, Clone)]
struct OctezCustomNetwork {
    pub chain_name: String,
    pub genesis: OctezGenesisChain,
    #[allow(dead_code)]
    pub sandboxed_chain_name: String,
    #[serde(default)]
    pub default_bootstrap_peers: Vec<String>,
    pub genesis_parameters: Option<OctezGenesisParameters>,
    #[serde(default)]
    pub user_activate_upgrades: Vec<UserActivatedProtocolUpgrades>,
    #[serde(default)]
    pub user_activate_protocol_overrides: Vec<UserActivatedProtocolOverride>,
}

#[derive(Deserialize, Debug, Clone)]
struct UserActivatedProtocolOverride {
    replaced_protocol: String,
    replacement_protocol: String,
}

#[derive(Deserialize, Debug, Clone)]
struct UserActivatedProtocolUpgrades {
    level: i32,
    replacement_protocol: String,
}

impl TryFrom<OctezCustomNetwork> for TezosEnvironmentConfiguration {
    type Error = TezosNetworkConfigurationError;

    fn try_from(octez: OctezCustomNetwork) -> Result<Self, Self::Error> {
        Ok(Self {
            genesis: octez.genesis.into(),
            bootstrap_lookup_addresses: octez.default_bootstrap_peers,
            version: octez.chain_name,
            protocol_overrides: ProtocolOverrides {
                user_activated_upgrades: octez
                    .user_activate_upgrades
                    .iter()
                    .map(|u| (u.level, u.replacement_protocol.clone()))
                    .collect(),
                user_activated_protocol_overrides: octez
                    .user_activate_protocol_overrides
                    .iter()
                    .map(|po| {
                        (
                            po.replaced_protocol.clone(),
                            po.replacement_protocol.clone(),
                        )
                    })
                    .collect(),
            },
            enable_testchain: false,
            patch_context_genesis_parameters: match octez.genesis_parameters {
                Some(gp) => Some(gp.try_into()?),
                None => None,
            },
        })
    }
}

#[derive(Deserialize, Debug, Clone)]
struct OctezGenesisChain {
    pub timestamp: String,
    pub block: String,
    pub protocol: String,
}

impl From<OctezGenesisChain> for GenesisChain {
    fn from(octez: OctezGenesisChain) -> Self {
        Self {
            time: octez.timestamp,
            block: octez.block,
            protocol: octez.protocol,
        }
    }
}

fn octez_default_context_key() -> String {
    "sandbox_parameter".to_owned()
}

#[derive(Deserialize, Debug, Clone)]
struct OctezGenesisParameters {
    #[serde(default = "octez_default_context_key")]
    context_key: String,
    #[serde(default)]
    values: HashMap<String, String>,
}

impl TryFrom<OctezGenesisParameters> for PatchContext {
    type Error = TezosNetworkConfigurationError;

    fn try_from(octez: OctezGenesisParameters) -> Result<Self, Self::Error> {
        Ok(Self {
            key: octez.context_key,
            json: serde_json::to_string(&octez.values)
                .map_err(|e| TezosNetworkConfigurationError::ParseError { reason: e })?,
        })
    }
}
