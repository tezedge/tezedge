use std::{path::PathBuf, io};

use thiserror::Error;
use derive_more::From;

use crypto::{
    hash::{PublicKeyEd25519, SecretKeyEd25519},
    base58::FromBase58CheckError, CryptoError,
};

#[derive(Debug, Error, From)]
pub enum ReadKeyError {
    #[error("{_0}")]
    Io(io::Error),
    #[error("{_0}")]
    De(serde_json::Error),
    #[error("key for {baker} not found")]
    KeyNotFound {
        baker: String,
    },
    #[error("failed to parse key")]
    FailedToParseKey,
    #[error("only unencrypted keys supported")]
    KeyEncrypted,
    #[error("only ed25519 keys supported")]
    UnsupportedKey,
    #[error("invalid key {_0}")]
    InvalidKey(FromBase58CheckError),
    #[error("crypto error {_0}")]
    Crypto(CryptoError),
}

pub fn read_key(base_dir: &PathBuf, baker: &str) -> Result<(PublicKeyEd25519, SecretKeyEd25519), ReadKeyError> {
    use std::fs::File;
    use serde::Deserialize;
    use crypto::hash::SeedEd25519;

    #[derive(Deserialize)]
    struct SecretKeyRecord {
        name: String,
        value: String,
    }

    let secret_keys = File::open(base_dir.join("secret_keys"))?;
    let secret_keys = serde_json::from_reader::<_, Vec<SecretKeyRecord>>(secret_keys)?;
    let secret_key = &secret_keys
        .iter()
        .find(|v| v.name == baker)
        .ok_or(ReadKeyError::KeyNotFound { baker: baker.to_owned() })?
        .value;
    let mut it = secret_key.split(':');
    let schema = it.next().ok_or(ReadKeyError::FailedToParseKey)?;
    if schema != "unencrypted" {
        return Err(ReadKeyError::KeyEncrypted);
    }
    let secret_key = it.next().ok_or(ReadKeyError::FailedToParseKey)?;
    if !secret_key.starts_with("edsk") {
        return Err(ReadKeyError::UnsupportedKey);
    }
    SeedEd25519::from_base58_check(secret_key)?.keypair().map_err(From::from)
}
