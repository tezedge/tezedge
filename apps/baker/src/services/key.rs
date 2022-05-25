// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{convert::TryFrom, io, path::PathBuf, str::FromStr, fs::File};

use derive_more::From;
use reqwest::{Url, blocking::Client};
use serde::Deserialize;
use thiserror::Error;

use crypto::{
    base58::FromBase58CheckError,
    hash::{ChainId, ContractTz1Hash, SecretKeyEd25519, Signature, TryFromPKError, SeedEd25519, Ed25519Signature},
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
    #[error("failed to parse key {_0}")]
    FailedToParseKey(SignerParseError),
}

#[derive(Debug, Error, From)]
pub enum SignError {
    #[error("{_0}")]
    Bin(BinError),
    #[error("{_0}")]
    Crypto(CryptoError),
    #[error("{_0}")]
    De(serde_json::Error),
    #[error("{_0}")]
    Reqwest(reqwest::Error),
    #[error("{_0}")]
    Base58(FromBase58CheckError),
}

pub struct CryptoService(Signer);

impl CryptoService {
    pub fn read_key(
        log: &slog::Logger,
        base_dir: &PathBuf,
        baker: &str,
    ) -> Result<Self, ReadKeyError> {
        #[derive(Deserialize)]
        struct SecretKeyRecord {
            name: String,
            value: String,
        }

        let secret_keys = File::open(base_dir.join("secret_keys"))?;
        let secret_keys = serde_json::from_reader::<_, Vec<SecretKeyRecord>>(secret_keys)?;
        let signer = secret_keys
            .iter()
            .find(|v| v.name == baker)
            .ok_or(ReadKeyError::KeyNotFound {
                baker: baker.to_string(),
            })?
            .value
            .parse::<Signer>()?;

        if let SignerBackend::RemoteHttps(_, url) = &signer.backend {
            slog::info!(log, "using remote signer: {}", url);
        } else {
            slog::info!(log, "using local key: {}", signer.pkh);
        }

        Ok(CryptoService(signer))
    }

    pub fn public_key_hash(&self) -> &ContractTz1Hash {
        &self.0.pkh
    }

    pub fn sign<T>(
        &self,
        watermark_tag: u8,
        chain_id: &ChainId,
        value: &T,
    ) -> Result<(Vec<u8>, Signature), SignError>
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
            .0
            .backend
            .sign(&[watermark_bytes.as_slice(), &value_bytes[..cut]])?;
        value_bytes.splice(cut.., signature.0.iter().cloned());
        Ok((value_bytes, signature))
    }
}

struct Signer {
    backend: SignerBackend,
    pkh: ContractTz1Hash,
}

enum SignerBackend {
    /// example: unencrypted:edsk4N...
    LiteralSecretKey(SecretKeyEd25519),
    // http also works here
    /// example: http://127.0.0.1:6732/keys/tz1TXkLKR4F4HUSCQKve7daPPLSxhZNx45px
    RemoteHttps(Client, Url),
    // example: tcp://127.0.0.1:7732/keys/tz1TXkLKR4F4HUSCQKve7daPPLSxhZNx45px
    // RemoteTcp(SocketAddr),
    // example: unix:/home/dev/.tezos-signer/socket?pkh=tz1TXkLKR4F4HUSCQKve7daPPLSxhZNx45px
    // UnixDomainSocket(PathBuf),
}

#[derive(Debug, Error, From)]
pub enum SignerParseError {
    #[error("schema not found")]
    NoSchema,
    #[error("unknown schema: \"{_0}\"")]
    UnknownSchema(String),
    #[error("only ed25519 keys supported")]
    UnsupportedKey,
    #[error("invalid key {_0}")]
    InvalidKey(FromBase58CheckError),
    #[error("crypto error {_0}")]
    Crypto(CryptoError),
    #[error("public key format error {_0}")]
    PkFormat(TryFromPKError),
    #[error("{_0}")]
    InvalidUrl(url::ParseError),
    #[error("missing \"/tz1...\"")]
    MissingPkhPathSegment,
}

impl FromStr for Signer {
    type Err = SignerParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split(':');
        let schema = it.next().ok_or(SignerParseError::NoSchema)?;
        let value = it.next().ok_or(SignerParseError::NoSchema)?;
        match schema {
            "unencrypted" => {
                if !value.starts_with("edsk") {
                    return Err(SignerParseError::UnsupportedKey);
                }
                let (public_key, secret_key) = SeedEd25519::from_base58_check(value)?
                    .keypair()?;

                Ok(Signer {
                    backend: SignerBackend::LiteralSecretKey(secret_key),
                    pkh: ContractTz1Hash::try_from(public_key.clone())?,
                })
            },
            "http" | "https" => {
                let url = Url::parse(s)?;
                let pkh_str = url.path_segments()
                    .ok_or(SignerParseError::MissingPkhPathSegment)?
                    .last()
                    .ok_or(SignerParseError::MissingPkhPathSegment)?;
                let pkh = ContractTz1Hash::from_base58_check(pkh_str)?;
                let client = Client::new();
                Ok(Signer {
                    backend: SignerBackend::RemoteHttps(client, url),
                    pkh,
                })
            },
            s => Err(SignerParseError::UnknownSchema(s.to_string())),
        }
    }
}

impl SignerBackend {
    pub fn sign<T, I>(&self, data: T) -> Result<Signature, SignError>
    where
        T: IntoIterator<Item = I>,
        I: AsRef<[u8]>,
    {
        match self {
            SignerBackend::LiteralSecretKey(sk) => sk.sign(data).map_err(SignError::Crypto),
            SignerBackend::RemoteHttps(client, url) => {
                let mut v = vec![];
                for d in data {
                    v.extend_from_slice(d.as_ref());
                }
                let body = format!("{:?}", hex::encode(v));
                let response = client
                    .post(url.clone())
                    .body(body)
                    .header("Content-Type", "application/json")
                    .send()?;

                #[derive(Deserialize)]
                struct SignerResponse {
                    signature: Ed25519Signature,
                }
                let SignerResponse { signature } = serde_json::from_reader(response)?;

                Ok(Signature(signature.0))
            }
        }
    }
}
