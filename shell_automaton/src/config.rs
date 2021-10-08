use std::time::SystemTime;

use hex::FromHex;
use serde::{Deserialize, Serialize};

use crate::shell_compatibility_version::ShellCompatibilityVersion;
use crypto::{
    crypto_box::{CryptoKey, PublicKey, SecretKey},
    hash::{CryptoboxPublicKeyHash, HashTrait},
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
    }
}
