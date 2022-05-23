// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{convert::TryFrom, io, path::PathBuf};

use derive_more::From;
use thiserror::Error;

use crypto::{
    base58::FromBase58CheckError,
    hash::{ChainId, ContractTz1Hash, SecretKeyEd25519, Signature, TryFromPKError},
    CryptoError,
};
use tezos_encoding::enc::{BinError, BinWriter};

#[derive(Debug, Error, From)]
pub enum ReadKeyError {
    #[error("{_0}")]
    Io(io::Error),
    #[error("{_0}")]
    De(serde_json::Error),
    #[error("key for {baker} not found")]
    KeyNotFound { baker: String },
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
    #[error("public key format error {_0}")]
    PkFormat(TryFromPKError),
}

#[derive(Debug, Error, From)]
pub enum EncodeError {
    #[error("{_0}")]
    Bin(BinError),
    #[error("{_0}")]
    Crypto(CryptoError),
}

pub struct CryptoService {
    secret_key: SecretKeyEd25519,
    public_key_hash: ContractTz1Hash,
}

impl CryptoService {
    pub fn read_key(base_dir: &PathBuf, baker: &str) -> Result<Self, ReadKeyError> {
        use crypto::hash::SeedEd25519;
        use serde::Deserialize;
        use std::fs::File;

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
            .ok_or(ReadKeyError::KeyNotFound {
                baker: baker.to_owned(),
            })?
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
        let (public_key, secret_key) = SeedEd25519::from_base58_check(secret_key)?
            .keypair()
            .map_err(ReadKeyError::Crypto)?;
        let public_key_hash = ContractTz1Hash::try_from(public_key.clone())?;

        Ok(CryptoService {
            secret_key,
            public_key_hash,
        })
    }

    pub fn public_key_hash(&self) -> &ContractTz1Hash {
        &self.public_key_hash
    }

    pub fn sign<T>(
        &self,
        watermark_tag: u8,
        chain_id: &ChainId,
        value: &T,
    ) -> Result<(Vec<u8>, Signature), EncodeError>
    where
        T: BinWriter,
    {
        let mut v = Vec::new();
        let mut value_bytes = {
            value.bin_write(&mut v)?;
            v
        };
        let watermark_bytes = {
            let mut v = Vec::with_capacity(5);
            v.push(watermark_tag);
            chain_id.bin_write(&mut v)?;
            v
        };
        let cut = value_bytes.len() - 64;
        let signature = self
            .secret_key
            .sign(&[watermark_bytes.as_slice(), &value_bytes[..cut]])?;
        value_bytes.splice(cut.., signature.0.iter().cloned());
        Ok((value_bytes, signature))
    }
}
