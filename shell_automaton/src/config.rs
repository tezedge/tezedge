// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{time::{Duration, SystemTime}, convert::TryFrom};

use hex::FromHex;
use serde::{Deserialize, Serialize};

use crate::shell_compatibility_version::ShellCompatibilityVersion;
use crypto::{
    crypto_box::{CryptoKey, PublicKey, SecretKey},
    hash::{CryptoboxPublicKeyHash, HashTrait, ChainId},
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
    pub peers_connected_max: usize,

    /// Disable automatic graylisting peers when something goes wrong.
    pub peers_graylist_disable: bool,

    /// Duration after which graylisted peer will timeout and be whitelisted.
    pub peers_graylist_timeout: Duration,

    /// If `Some(interval)` record/persist state snapshots to storage
    /// every `interval` actions.
    /// If `None`, only persist initial state snapshot.
    pub record_state_snapshots_with_interval: Option<u64>,

    /// Record/Persist actions.
    pub record_actions: bool,

    pub quota: Quota,
}

impl Config {
    pub fn min_time_interval(&self) -> Duration {
        self.check_timeouts_interval
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Quota {
    pub restore_duration_millis: u64,
    pub read_quota: usize,
    pub write_quota: usize,
}

pub fn default_config() -> Config {
    let pow_target = 26.0;
    Config {
        initial_time: SystemTime::now(),

        port: 9732,
        disable_mempool: false,
        private_node: false,
        pow_target,
        // identity: Identity::generate(pow_target).unwrap(),
        identity: identity_1(),
        shell_compatibility_version: ShellCompatibilityVersion::new(
            "TEZOS_MAINNET".to_owned(),
            vec![0, 1],
            vec![1],
        ),
        chain_id: ChainId::try_from("NetXdQprcVkpaWU").unwrap(),

        check_timeouts_interval: Duration::from_millis(100),
        peer_connecting_timeout: Duration::from_secs(4),
        peer_handshaking_timeout: Duration::from_secs(8),

        peer_max_io_syscalls: 64,

        peers_potential_max: 80,
        peers_connected_max: 40,

        peers_graylist_disable: false,
        peers_graylist_timeout: Duration::from_secs(15 * 60),

        record_state_snapshots_with_interval: None,
        record_actions: false,

        quota: Quota {
            restore_duration_millis: 1000,
            read_quota: 1024,
            write_quota: 1024,
        },
    }
}

pub fn test_config() -> Config {
    let pow_target = 0.0;
    Config {
        initial_time: SystemTime::now(),

        port: 19732,
        disable_mempool: false,
        private_node: false,
        pow_target,
        // identity: Identity::generate(pow_target).unwrap(),
        identity: identity_1(),
        shell_compatibility_version: ShellCompatibilityVersion::new(
            "TEZOS_GRANADANET_2021-05-21T15:00:00Z".to_owned(),
            vec![0],
            vec![1],
        ),
        chain_id: ChainId::try_from("NetXz969SFaFn8k").unwrap(),

        check_timeouts_interval: Duration::from_millis(100),
        peer_connecting_timeout: Duration::from_secs(4),
        peer_handshaking_timeout: Duration::from_secs(8),

        peer_max_io_syscalls: 64,

        peers_potential_max: 80,
        peers_connected_max: 40,

        peers_graylist_disable: false,
        peers_graylist_timeout: Duration::from_secs(15 * 60),

        record_state_snapshots_with_interval: None,
        record_actions: false,

        quota: Quota {
            restore_duration_millis: 1000,
            read_quota: 1024,
            write_quota: 1024,
        },
    }
}
