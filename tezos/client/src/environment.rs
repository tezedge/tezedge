// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use tezos_interop::ffi::GenesisChain;

use custom_derive::custom_derive;
use enum_derive::{enum_derive_util, EnumFromStr, IterVariants};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref TEZOS_ENV: HashMap<TezosEnvironment, TezosEnvironmentConfiguration> = init();
}

custom_derive! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, EnumFromStr, IterVariants(TezosEnvironmentVariants))]
    pub enum TezosEnvironment {
        Alphanet,
        Babylonnet,
        Mainnet,
        Zeronet
    }
}

/// Initializes hard-code configuration according to different Tezos git branches (genesis_chain.ml, node_config_file.ml)
fn init() -> HashMap<TezosEnvironment, TezosEnvironmentConfiguration> {
    let mut env: HashMap<TezosEnvironment, TezosEnvironmentConfiguration> = HashMap::new();

    env.insert(TezosEnvironment::Alphanet, TezosEnvironmentConfiguration {
        genesis: TezosGenesisChain {
            time: "2018-11-30T15:30:56Z".to_string(),
            block: "BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe".to_string(),
            protocol: "Ps6mwMrF2ER2s51cp9yYpjDcuzQjsc2yAz8bQsRgdaRxw4Fk95H".to_string(),
        },
        bootstrap_lookup_addresses: vec![
            "boot.tzalpha.net".to_string(),
            "bootalpha.tzbeta.net".to_string()
        ],
        version: "TEZOS_ALPHANET_2018-11-30T15:30:56Z".to_string()
    });

    env.insert(TezosEnvironment::Babylonnet, TezosEnvironmentConfiguration {
        genesis: TezosGenesisChain {
            time: "2019-09-27T07:43:32Z".to_string(),
            block: "BLockGenesisGenesisGenesisGenesisGenesisd1f7bcGMoXy".to_string(),
            protocol: "PtBMwNZT94N7gXKw4i273CKcSaBrrBnqnt3RATExNKr9KNX2USV".to_string(),
        },
        bootstrap_lookup_addresses: vec![
            "35.246.251.120".to_string(),
            "34.89.154.253".to_string(),
            "babylonnet.kaml.fr".to_string(),
            "tezaria.com".to_string()
        ],
        version: "TEZOS_ALPHANET_BABYLON_2019-09-27T07:43:32Z".to_string()
    });

    env.insert(TezosEnvironment::Mainnet, TezosEnvironmentConfiguration {
        genesis: TezosGenesisChain {
            time: "2018-06-30T16:07:32Z".to_string(),
            block: "BLockGenesisGenesisGenesisGenesisGenesisf79b5d1CoW2".to_string(),
            protocol: "Ps9mPmXaRzmzk35gbAYNCAw6UXdE2qoABTHbN2oEEc1qM7CwT9P".to_string(),
        },
        bootstrap_lookup_addresses: vec![
            "boot.tzbeta.net".to_string()
        ],
        version: "TEZOS_BETANET_2018-06-30T16:07:32Z".to_string()
    });

    env.insert(TezosEnvironment::Zeronet, TezosEnvironmentConfiguration {
        genesis: TezosGenesisChain {
            time: "2019-08-06T15:18:56Z".to_string(),
            block: "BLockGenesisGenesisGenesisGenesisGenesiscde8db4cX94".to_string(),
            protocol: "PtBMwNZT94N7gXKw4i273CKcSaBrrBnqnt3RATExNKr9KNX2USV".to_string(),
        },
        bootstrap_lookup_addresses: vec![
            "bootstrap.zeronet.fun".to_string(),
            "bootzero.tzbeta.net".to_string()
        ],
        version: "TEZOS_ZERONET_2019-08-06T15:18:56Z".to_string()
    });

    env
}

pub type TezosGenesisChain = GenesisChain;

pub struct TezosEnvironmentConfiguration {
    pub genesis: TezosGenesisChain,
    pub bootstrap_lookup_addresses: Vec<String>,
    pub version: String,
}
