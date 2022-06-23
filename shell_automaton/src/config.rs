// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;
use std::time::{Duration, SystemTime};

use hex::FromHex;
use serde::{Deserialize, Serialize};
use storage::StorageInitInfo;
use tezos_api::{environment::TezosEnvironmentConfiguration, ffi::TezosRuntimeConfiguration};
use tezos_context_api::{
    ContextKvStoreConfiguration, GenesisChain, ProtocolOverrides, TezosContextStorageConfiguration,
    TezosContextTezEdgeStorageConfiguration, TezosContextTezedgeOnDiskBackendOptions,
};
use tezos_messages::base::signature_public_key::SignaturePublicKeyHash;
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_protocol_ipc_client::ProtocolRunnerConfiguration;

use crate::baker::block_baker::LiquidityBakingToggleVote;
use crate::shell_compatibility_version::ShellCompatibilityVersion;
use crypto::{
    crypto_box::{CryptoKey, PublicKey, SecretKey},
    hash::{BlockHash, ChainId, CryptoboxPublicKeyHash, HashTrait},
    proof_of_work::ProofOfWork,
};
use tezos_identity::Identity;

use crate::Port;

pub fn identity(pkh: &[u8], pk: &[u8], sk: &[u8], pow: &[u8]) -> Identity {
    Identity {
        peer_id: CryptoboxPublicKeyHash::try_from_bytes(pkh).unwrap(),
        public_key: PublicKey::from_bytes(pk).unwrap(),
        secret_key: SecretKey::from_bytes(sk).unwrap(),
        proof_of_work_stamp: ProofOfWork::from_hex(hex::encode(pow)).unwrap(),
    }
}

pub fn identity_1() -> Identity {
    const IDENTITY: &str = r#"
{
    "peer_id": "idrdoT9g6YwELhUQyshCcHwAzBS9zA",
    "proof_of_work_stamp": "bbc2300149249e1ccc84a54362236c3cbbc2cc2ffbd3b6ea",
    "public_key": "94498d9416140fbc458495333daac1b4c87e419f5726717a54f9b6c67476ae1c",
    "secret_key": "ac7acf3afed7637be10f8fc76a2eb6b3359c78adb1d813b41cbab3fae954f4b1"
}
"#;
    Identity::from_json(IDENTITY).unwrap()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub initial_time: SystemTime,

    pub protocol_runner: ProtocolRunnerConfiguration,
    pub init_storage_data: StorageInitInfo,

    pub port: Port,
    pub disable_mempool: bool,
    pub private_node: bool,
    pub pow_target: f64,
    pub identity: Identity,
    pub shell_compatibility_version: ShellCompatibilityVersion,
    pub chain_id: ChainId,

    /// How often to check for timeouts.
    ///
    /// E.g. if it's set to 100ms, every 100ms we will check state for timeouts.
    pub check_timeouts_interval: Duration,

    /// Peers addresses to do dns lookup on to discover initial peers
    /// to connect to.
    pub peers_dns_lookup_addresses: Vec<(String, Port)>,

    /// Timeout for peer tcp socket connection.
    pub peer_connecting_timeout: Duration,

    /// Timeout for the whole handshaking process. If handshaking isn't
    /// done within this time limitation, we will disconnect and blacklist the peer.
    pub peer_handshaking_timeout: Duration,

    /// Limit for the number of allowed syscalls in one iteration/loop ("in one go").
    pub peer_max_io_syscalls: usize,

    /// Maximum number of potential peers.
    pub peers_potential_max: usize,

    /// Maximum number of connected peers on socket layer.
    pub peers_connected_min: usize,

    /// Maximum number of connected peers on socket layer.
    pub peers_connected_max: usize,

    /// Minimum number of peers that we need to be bootstrapped with
    /// (our current head equal or greater than theirs),
    /// for us to be considered bootstrapped (`Self::is_bootstrapped() == true`).
    pub peers_bootstrapped_min: usize,

    /// Disable automatic graylisting peers when something goes wrong.
    pub peers_graylist_disable: bool,

    /// Duration after which graylisted peer will timeout and be whitelisted.
    pub peers_graylist_timeout: Duration,

    pub bootstrap_block_header_get_timeout: Duration,
    pub bootstrap_block_operations_get_timeout: Duration,

    /// Override level that we will start bootstrapping from.
    /// If we don't have block with such level in storage, we will use
    /// actuall current head stored in storage instead.
    pub current_head_level_override: Option<Level>,

    /// If `Some(interval)` record/persist state snapshots to storage
    /// every `interval` actions.
    /// If `None`, only persist initial state snapshot.
    pub record_state_snapshots_with_interval: Option<u64>,

    /// Record/Persist actions.
    pub record_actions: bool,

    pub disable_block_precheck: bool,
    pub disable_endorsements_precheck: bool,

    pub mempool_get_operation_timeout: Duration,

    pub bakers: Vec<BakerConfig>,
}

impl Config {
    pub fn min_time_interval(&self) -> Duration {
        self.check_timeouts_interval
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerConfig {
    pub pkh: SignaturePublicKeyHash,
    pub liquidity_baking_escape_vote: LiquidityBakingToggleVote,
}

pub fn default_test_config() -> Config {
    Config {
        initial_time: SystemTime::now(),

        protocol_runner: ProtocolRunnerConfiguration {
            runtime_configuration: TezosRuntimeConfiguration {
                log_enabled: false,
                log_level: None,
            },
            environment: TezosEnvironmentConfiguration {
                genesis: GenesisChain {
                    time: "2018-06-30T16:07:32Z".to_string(),
                    block: "BLockGenesisGenesisGenesisGenesisGenesisf79b5d1CoW2".to_string(),
                    protocol: "Ps9mPmXaRzmzk35gbAYNCAw6UXdE2qoABTHbN2oEEc1qM7CwT9P".to_string(),
                },
                bootstrap_lookup_addresses: vec![],
                version: "".to_owned(),
                protocol_overrides: ProtocolOverrides {
                    user_activated_upgrades: vec![],
                    user_activated_protocol_overrides: vec![],
                },
                enable_testchain: false,
                patch_context_genesis_parameters: None,
            },
            enable_testchain: false,
            storage: TezosContextStorageConfiguration::TezEdgeOnly(
                TezosContextTezEdgeStorageConfiguration {
                    backend: ContextKvStoreConfiguration::InMem(
                        TezosContextTezedgeOnDiskBackendOptions {
                            base_path: "/tmp/tezedge".to_string(),
                            startup_check: false,
                        },
                    ),
                    ipc_socket_path: None,
                },
            ),
            executable_path: Default::default(),
            log_level: slog::Level::Error,
        },
        init_storage_data: StorageInitInfo {
            chain_id: ChainId::try_from_bytes(&[122, 6, 167, 112]).unwrap(),
            genesis_block_header_hash: BlockHash::from_base58_check(
                "BLockGenesisGenesisGenesisGenesisGenesisf79b5d1CoW2",
            )
            .unwrap(),
            patch_context: None,
            context_stats_db_path: None,
            replay: None,
        },

        port: 9732,
        disable_mempool: false,
        private_node: false,
        pow_target: 0.0,
        // identity: Identity::generate(pow_target).unwrap(),
        identity: identity_1(),
        shell_compatibility_version: ShellCompatibilityVersion::new(
            "TEZOS_MAINNET".to_owned(),
            vec![0, 1],
            vec![1],
        ),
        chain_id: ChainId::try_from("NetXdQprcVkpaWU").unwrap(),

        check_timeouts_interval: Duration::from_millis(100),

        peers_dns_lookup_addresses: vec![],
        peer_connecting_timeout: Duration::from_secs(4),
        peer_handshaking_timeout: Duration::from_secs(8),

        peer_max_io_syscalls: 64,

        peers_potential_max: 80,
        peers_connected_min: 20,
        peers_connected_max: 40,
        peers_bootstrapped_min: 60,

        peers_graylist_disable: false,
        peers_graylist_timeout: Duration::from_secs(15 * 60),

        bootstrap_block_header_get_timeout: Duration::from_millis(500),
        bootstrap_block_operations_get_timeout: Duration::from_millis(1000),

        current_head_level_override: None,

        record_state_snapshots_with_interval: None,
        record_actions: false,

        disable_endorsements_precheck: true,
        disable_block_precheck: true,

        mempool_get_operation_timeout: Duration::from_secs(1),

        bakers: vec![],
    }
}
