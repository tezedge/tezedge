// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::str::FromStr;

use chrono::prelude::*;
use chrono::ParseError;
use enum_iterator::IntoEnumIterator;
use failure::Fail;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use crypto::base58::FromBase58CheckError;
use crypto::hash::{
    chain_id_from_block_hash, BlockHash, ChainId, ContextHash, HashType, OperationListListHash,
    ProtocolHash,
};
use tezos_messages::p2p::encoding::prelude::{BlockHeader, BlockHeaderBuilder};

use crate::ffi::{GenesisChain, PatchContext, ProtocolOverrides};

lazy_static! {
    pub static ref TEZOS_ENV: HashMap<TezosEnvironment, TezosEnvironmentConfiguration> = init();
    /// alternative to ocaml Operation_list_list_hash.empty
    pub static ref OPERATION_LIST_LIST_HASH_EMPTY: OperationListListHash = HashType::OperationListListHash.b58check_to_hash("LLoZS2LW3rEi7KYU4ouBQtorua37aWWCtpDmv1n2x3xoKi6sVXLWp").unwrap();
}

/// Enum representing different Tezos environment.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq, Hash, IntoEnumIterator)]
pub enum TezosEnvironment {
    Alphanet,
    Babylonnet,
    Carthagenet,
    Delphinet,
    Edonet,
    Mainnet,
    Zeronet,
    Sandbox,
}

#[derive(Debug, Clone)]
pub struct ParseTezosEnvironmentError(String);

impl FromStr for TezosEnvironment {
    type Err = ParseTezosEnvironmentError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "alphanet" => Ok(TezosEnvironment::Alphanet),
            "babylonnet" | "babylon" => Ok(TezosEnvironment::Babylonnet),
            "carthagenet" | "carthage" => Ok(TezosEnvironment::Carthagenet),
            "delphinet" | "delphi" => Ok(TezosEnvironment::Delphinet),
            "edonet" | "edo" => Ok(TezosEnvironment::Edonet),
            "mainnet" => Ok(TezosEnvironment::Mainnet),
            "zeronet" => Ok(TezosEnvironment::Zeronet),
            "sandbox" => Ok(TezosEnvironment::Sandbox),
            _ => Err(ParseTezosEnvironmentError(format!(
                "Invalid variant name: {}",
                s
            ))),
        }
    }
}

/// Initializes hard-code configuration according to different Tezos git branches (genesis_chain.ml, node_config_file.ml)
fn init() -> HashMap<TezosEnvironment, TezosEnvironmentConfiguration> {
    let mut env: HashMap<TezosEnvironment, TezosEnvironmentConfiguration> = HashMap::new();

    env.insert(
        TezosEnvironment::Alphanet,
        TezosEnvironmentConfiguration {
            genesis: GenesisChain {
                time: "2018-11-30T15:30:56Z".to_string(),
                block: "BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe".to_string(),
                protocol: "Ps6mwMrF2ER2s51cp9yYpjDcuzQjsc2yAz8bQsRgdaRxw4Fk95H".to_string(),
            },
            bootstrap_lookup_addresses: vec![
                "boot.tzalpha.net".to_string(),
                "bootalpha.tzbeta.net".to_string(),
            ],
            version: "TEZOS_ALPHANET_2018-11-30T15:30:56Z".to_string(),
            protocol_overrides: ProtocolOverrides {
                user_activated_upgrades: vec![],
                user_activated_protocol_overrides: vec![],
            },
            enable_testchain: false,
            patch_context_genesis_parameters: None,
        },
    );

    env.insert(
        TezosEnvironment::Babylonnet,
        TezosEnvironmentConfiguration {
            genesis: GenesisChain {
                time: "2019-09-27T07:43:32Z".to_string(),
                block: "BLockGenesisGenesisGenesisGenesisGenesisd1f7bcGMoXy".to_string(),
                protocol: "PtBMwNZT94N7gXKw4i273CKcSaBrrBnqnt3RATExNKr9KNX2USV".to_string(),
            },
            bootstrap_lookup_addresses: vec![
                "35.246.251.120".to_string(),
                "34.89.154.253".to_string(),
                "babylonnet.kaml.fr".to_string(),
            ],
            version: "TEZOS_ALPHANET_BABYLON_2019-09-27T07:43:32Z".to_string(),
            protocol_overrides: ProtocolOverrides {
                user_activated_upgrades: vec![],
                user_activated_protocol_overrides: vec![],
            },
            enable_testchain: true,
            patch_context_genesis_parameters: None,
        },
    );

    env.insert(
        TezosEnvironment::Carthagenet,
        TezosEnvironmentConfiguration {
            genesis: GenesisChain {
                time: "2019-11-28T13:02:13Z".to_string(),
                block: "BLockGenesisGenesisGenesisGenesisGenesisd6f5afWyME7".to_string(),
                protocol: "PtYuensgYBb3G3x1hLLbCmcav8ue8Kyd2khADcL5LsT5R1hcXex".to_string(),
            },
            bootstrap_lookup_addresses: vec![
                "34.76.169.218".to_string(),
                "34.90.24.160".to_string(),
                "carthagenet.kaml.fr".to_string(),
                "104.248.136.94".to_string(),
            ],
            version: "TEZOS_ALPHANET_CARTHAGE_2019-11-28T13:02:13Z".to_string(),
            protocol_overrides: ProtocolOverrides {
                user_activated_upgrades: vec![],
                user_activated_protocol_overrides: vec![],
            },
            enable_testchain: true,
            patch_context_genesis_parameters: None,
        },
    );

    env.insert(TezosEnvironment::Delphinet, TezosEnvironmentConfiguration {
        genesis: GenesisChain {
            time: "2020-09-04T07:08:53Z".to_string(),
            block: "BLockGenesisGenesisGenesisGenesisGenesis355e8bjkYPv".to_string(),
            protocol: "PtYuensgYBb3G3x1hLLbCmcav8ue8Kyd2khADcL5LsT5R1hcXex".to_string(),
        },
        bootstrap_lookup_addresses: vec![
            "delphinet.tezos.co.il".to_string(),
            "delphinet.smartpy.io".to_string(),
            "delphinet.kaml.fr".to_string(),
            "13.53.41.201".to_string(),
        ],
        version: "TEZOS_DELPHINET_2020-09-04T07:08:53Z".to_string(),
        protocol_overrides: ProtocolOverrides {
            user_activated_upgrades: vec![],
            user_activated_protocol_overrides: vec![],
        },
        enable_testchain: true,
        patch_context_genesis_parameters: Some(PatchContext {
            key: "sandbox_parameter".to_string(),
            json: r#"{ "genesis_pubkey": "edpkugeDwmwuwyyD3Q5enapgEYDxZLtEUFFSrvVwXASQMVEqsvTqWu" }"#.to_string(),
        }),
    });

    env.insert(TezosEnvironment::Edonet, TezosEnvironmentConfiguration {
        genesis: GenesisChain {
            time: "2020-11-30T12:00:00Z".to_string(),
            block: "BLockGenesisGenesisGenesisGenesisGenesis2431bbUwV2a".to_string(),
            protocol: "PtYuensgYBb3G3x1hLLbCmcav8ue8Kyd2khADcL5LsT5R1hcXex".to_string(),
        },
        bootstrap_lookup_addresses: vec![
            "51.75.246.56:9733".to_string(),
            "edonet.tezos.co.il".to_string(),
            "46.245.179.161:9733".to_string(),
            "edonet.smartpy.io".to_string(),
            "188.40.128.216:29732".to_string(),
            "51.79.165.131".to_string(),
            "edonet.boot.tezostaquito.io".to_string(),
            "95.216.228.228:9733".to_string(),
        ],
        version: "TEZOS_EDONET_2020-11-30T12:00:00Z".to_string(),
        protocol_overrides: ProtocolOverrides {
            user_activated_upgrades: vec![],
            user_activated_protocol_overrides: vec![],
        },
        enable_testchain: true,
        patch_context_genesis_parameters: Some(PatchContext {
            key: "sandbox_parameter".to_string(),
            json: r#"{ "genesis_pubkey": "edpkugeDwmwuwyyD3Q5enapgEYDxZLtEUFFSrvVwXASQMVEqsvTqWu" }"#.to_string(),
        }),
    });

    env.insert(
        TezosEnvironment::Mainnet,
        TezosEnvironmentConfiguration {
            genesis: GenesisChain {
                time: "2018-06-30T16:07:32Z".to_string(),
                block: "BLockGenesisGenesisGenesisGenesisGenesisf79b5d1CoW2".to_string(),
                protocol: "Ps9mPmXaRzmzk35gbAYNCAw6UXdE2qoABTHbN2oEEc1qM7CwT9P".to_string(),
            },
            bootstrap_lookup_addresses: vec![
                "boot.tzbeta.net".to_string(),
                "116.202.172.21".to_string(), /* giganode_1 */
                "95.216.45.62".to_string(),   /* giganode_2 */
            ],
            version: "TEZOS_MAINNET".to_string(),
            protocol_overrides: ProtocolOverrides {
                user_activated_upgrades: vec![
                    (
                        28082_i32,
                        "PsYLVpVvgbLhAhoqAkMFUo6gudkJ9weNXhUYCiLDzcUpFpkk8Wt".to_string(),
                    ),
                    (
                        204761_i32,
                        "PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP".to_string(),
                    ),
                ],
                user_activated_protocol_overrides: vec![(
                    "PsBABY5HQTSkA4297zNHfsZNKtxULfL18y95qb3m53QJiXGmrbU".to_string(),
                    "PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS".to_string(),
                )],
            },
            enable_testchain: false,
            patch_context_genesis_parameters: None,
        },
    );

    env.insert(
        TezosEnvironment::Zeronet,
        TezosEnvironmentConfiguration {
            genesis: GenesisChain {
                time: "2019-08-06T15:18:56Z".to_string(),
                block: "BLockGenesisGenesisGenesisGenesisGenesiscde8db4cX94".to_string(),
                protocol: "PtBMwNZT94N7gXKw4i273CKcSaBrrBnqnt3RATExNKr9KNX2USV".to_string(),
            },
            bootstrap_lookup_addresses: vec![
                "bootstrap.zeronet.fun".to_string(),
                "bootzero.tzbeta.net".to_string(),
            ],
            version: "TEZOS_ZERONET_2019-08-06T15:18:56Z".to_string(),
            protocol_overrides: ProtocolOverrides {
                user_activated_upgrades: vec![],
                user_activated_protocol_overrides: vec![],
            },
            enable_testchain: true,
            patch_context_genesis_parameters: None,
        },
    );

    env.insert(TezosEnvironment::Sandbox, TezosEnvironmentConfiguration {
        genesis: GenesisChain {
            time: "2018-06-30T16:07:32Z".to_string(),
            block: "BLockGenesisGenesisGenesisGenesisGenesisf79b5d1CoW2".to_string(),
            protocol: "ProtoGenesisGenesisGenesisGenesisGenesisGenesk612im".to_string(),
        },
        bootstrap_lookup_addresses: vec![],
        version: "SANDBOXED_TEZOS".to_string(),
        protocol_overrides: ProtocolOverrides {
            user_activated_upgrades: vec![],
            user_activated_protocol_overrides: vec![],
        },
        enable_testchain: false,
        patch_context_genesis_parameters: Some(PatchContext {
            key: "sandbox_parameter".to_string(),
            json: r#"{ "genesis_pubkey": "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2" }"#.to_string(),
        }),
    });

    env
}

/// Possible errors for environment
#[derive(Debug, Fail)]
pub enum TezosEnvironmentError {
    #[fail(display = "Invalid block hash: {}, reason: {:?}", hash, error)]
    InvalidBlockHash {
        hash: String,
        error: FromBase58CheckError,
    },
    #[fail(display = "Invalid protocol hash: {}, reason: {:?}", hash, error)]
    InvalidProtocolHash {
        hash: String,
        error: FromBase58CheckError,
    },
    #[fail(display = "Invalid time: {}, reason: {:?}", time, error)]
    InvalidTime { time: String, error: ParseError },
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GenesisAdditionalData {
    pub max_operations_ttl: u16,
    pub last_allowed_fork_level: i32,
}

/// Structure holding all environment specific crucial information - according to different Tezos Gitlab branches
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TezosEnvironmentConfiguration {
    /// Genesis information - see genesis_chain.ml
    pub genesis: GenesisChain,
    /// Adresses used for initial bootstrap, if no peers are configured - see node_config_file.ml
    pub bootstrap_lookup_addresses: Vec<String>,
    /// chain_name - see distributed_db_version.ml
    pub version: String,
    /// protocol overrides - see block_header.ml
    pub protocol_overrides: ProtocolOverrides,
    /// if network has enabled switching test chains by default
    pub enable_testchain: bool,
    /// some networks could require patching context for genesis - like to change genesis key...
    /// (also this can be overriden on startup with cmd args)
    pub patch_context_genesis_parameters: Option<PatchContext>,
}

impl TezosEnvironmentConfiguration {
    /// Resolves genesis hash from configuration of GenesisChain.block
    pub fn genesis_header_hash(&self) -> Result<BlockHash, TezosEnvironmentError> {
        HashType::BlockHash
            .b58check_to_hash(&self.genesis.block)
            .map_err(|e| TezosEnvironmentError::InvalidBlockHash {
                hash: self.genesis.block.clone(),
                error: e,
            })
    }

    /// Resolves main chain_id, which is computed from genesis header
    pub fn main_chain_id(&self) -> Result<ChainId, TezosEnvironmentError> {
        Ok(chain_id_from_block_hash(&self.genesis_header_hash()?))
    }

    /// Resolves genesis protocol
    pub fn genesis_protocol(&self) -> Result<ProtocolHash, TezosEnvironmentError> {
        HashType::ProtocolHash
            .b58check_to_hash(&self.genesis.protocol)
            .map_err(|e| TezosEnvironmentError::InvalidProtocolHash {
                hash: self.genesis.protocol.clone(),
                error: e,
            })
    }

    pub fn genesis_time(&self) -> Result<i64, TezosEnvironmentError> {
        parse_from_rfc3339(&self.genesis.time)
    }

    /// Returns initialized default genesis header
    pub fn genesis_header(
        &self,
        context_hash: ContextHash,
        operation_list_list_hash: OperationListListHash,
    ) -> Result<BlockHeader, TezosEnvironmentError> {
        // genesis predecessor is genesis
        let genesis_hash = self.genesis_header_hash()?;
        let genesis_time: i64 = self.genesis_time()?;

        Ok(BlockHeaderBuilder::default()
            .level(0)
            .proto(0)
            .predecessor(genesis_hash)
            .timestamp(genesis_time)
            .validation_pass(0)
            .operations_hash(operation_list_list_hash)
            .fitness(vec![])
            .context(context_hash)
            .protocol_data(vec![])
            .build()
            .unwrap())
    }

    pub fn genesis_additional_data(&self) -> GenesisAdditionalData {
        GenesisAdditionalData {
            max_operations_ttl: 0,
            last_allowed_fork_level: 0,
        }
    }
}

fn parse_from_rfc3339(time: &str) -> Result<i64, TezosEnvironmentError> {
    DateTime::parse_from_rfc3339(time)
        .map_err(|e| TezosEnvironmentError::InvalidTime {
            time: time.to_string(),
            error: e,
        })
        .map(|dt| dt.timestamp())
}

#[cfg(test)]
mod tests {
    use tezos_messages::ts_to_rfc3339;

    use super::*;

    #[test]
    fn encoded_decoded_timestamp() -> Result<(), failure::Error> {
        let dt = parse_from_rfc3339("2019-11-28T13:02:13Z")?;
        let decoded = ts_to_rfc3339(dt);
        let expected = "2019-11-28T13:02:13Z".to_string();

        assert_eq!(expected, decoded);
        Ok(())
    }
}
