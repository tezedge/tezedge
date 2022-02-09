// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{cell::Cell, io, str, sync::mpsc, thread, time::Duration, convert::TryInto};

use chrono::{DateTime, Utc};
use derive_more::From;
use reqwest::{
    blocking::{Client, Response},
    StatusCode, Url,
};
use serde::{Deserialize, Serialize};
use slog::Logger;
use tezos_encoding::types::SizedBytes;
use thiserror::Error;

use crypto::hash::{BlockHash, ChainId, ContractTz1Hash, SecretKeyEd25519, Signature};
use tezos_messages::{
    p2p::encoding::operation::DecodedOperation,
    protocol::proto_012::operation::FullHeader,
};

use super::types::{ProtocolBlockHeader, ShellBlockHeader, sign_any};

#[derive(Debug, Error, From)]
pub enum TezosClientError {
    #[error("{_0}")]
    Reqwest(reqwest::Error),
    #[error("{_0}")]
    SerdeJson(serde_json::Error),
    #[error("{_0}")]
    Io(io::Error),
    #[error("{_0}")]
    Utf8(str::Utf8Error),
    #[error("{_0}")]
    Custom(String),
}

#[derive(Debug)]
pub enum TezosClientEvent {
    NewHead(serde_json::Value),
    Operation(serde_json::Value),
}

pub struct TezosClient {
    tx: mpsc::Sender<TezosClientEvent>,
    endpoint: Url,
    inner: Client,
    counter: Cell<usize>,
    log: Logger,
    main_logger: Logger,
}

#[derive(Deserialize)]
pub struct Constants {
    pub consensus_committee_size: u32,
    pub minimal_block_delay: String,
    pub delay_increment_per_round: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct BlockHeader {
    pub level: i32,
    pub hash: BlockHash,
    pub predecessor: BlockHash,
    pub protocol_data: String,

    pub proto: u8,
    pub timestamp: String,
    validation_pass: u8,
    operations_hash: String,
    fitness: Vec<String>,
    context: String,
}

#[derive(Deserialize)]
pub struct Validator {
    pub level: u32,
    pub delegate: ContractTz1Hash,
    pub slots: Vec<u16>,
}

#[derive(Deserialize, Clone)]
pub struct BakingRights {
    pub level: i32,
    pub delegate: ContractTz1Hash,
    pub round: u32,
    pub estimated_time: Option<String>,
}

impl TezosClient {
    // 012-Psithaca
    pub const PROTOCOL: &'static str = "Psithaca2MLRFYargivpo7YvUr7wUDqyxrdhC5CQq78mRvimz6A";

    pub fn new(
        log: Logger,
        main_logger: Logger,
        endpoint: Url,
    ) -> (Self, mpsc::Receiver<TezosClientEvent>) {
        let (tx, rx) = mpsc::channel();
        (
            TezosClient {
                tx,
                endpoint,
                inner: Client::new(),
                counter: Cell::new(0),
                log,
                main_logger,
            },
            rx,
        )
    }

    fn request_inner(
        &self,
        url: Url,
        timeout: Option<Duration>,
    ) -> reqwest::Result<(Response, usize, StatusCode)> {
        let counter = self.counter.get();
        self.counter.set(counter + 1);
        slog::info!(self.log, ">>>>{}: {}", counter, url);
        let request = self.inner.get(url);
        let request = if let Some(timeout) = timeout {
            request.timeout(timeout)
        } else {
            request
        };
        let response = request.send()?;
        let status = response.status();
        Ok((response, counter, status))
    }

    /// spawning a thread
    #[allow(dead_code)]
    pub fn spawn_monitor_main_head(&self) -> Result<thread::JoinHandle<()>, TezosClientError> {
        let mut url = self
            .endpoint
            .join("monitor/heads/main")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("next_protocol", Self::PROTOCOL);
        self.spawn_monitor(url, TezosClientEvent::NewHead)
    }

    /// spawning a thread
    #[allow(dead_code)]
    pub fn spawn_monitor_operations(&self) -> Result<thread::JoinHandle<()>, TezosClientError> {
        let mut url = self
            .endpoint
            .join("chains/main/mempool/monitor_operations")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("applied", "yes")
            .append_pair("refused", "no")
            .append_pair("outdated", "no")
            .append_pair("branch_refused", "no")
            .append_pair("branch_delayed", "yes");
        self.spawn_monitor(url, TezosClientEvent::Operation)
    }

    #[allow(dead_code)]
    fn spawn_monitor<F>(
        &self,
        url: Url,
        wrapper: F,
    ) -> Result<thread::JoinHandle<()>, TezosClientError>
    where
        F: Fn(serde_json::Value) -> TezosClientEvent + Send + 'static,
    {
        let (response, counter, status) = self.request_inner(url, None)?;

        let mut deserializer =
            serde_json::Deserializer::from_reader(response).into_iter::<serde_json::Value>();

        let log = self.log.clone();
        let main_logger = self.main_logger.clone();
        let tx = self.tx.clone();
        let handle = thread::Builder::new()
            .spawn(move || {
                while let Some(v) = deserializer.next() {
                    match v {
                        Ok(value) => {
                            if let Some(arr) = value.as_array() {
                                if arr.is_empty() {
                                    continue;
                                }
                            }
                            slog::info!(log, "<<<<{}: {}", counter, status);
                            slog::info!(log, "{}", value);
                            if let Err(_) = tx.send(wrapper(value)) {
                                slog::error!(log, "receiver is disconnected");
                            }
                        }
                        Err(err) => {
                            slog::info!(log, "<<<<{}: {}", counter, status);
                            slog::error!(main_logger, "{status}, {err}");
                        }
                    }
                }
            })
            .expect("valid thread name");
        Ok(handle)
    }

    pub fn inject_block(
        &self,
        secret_key: &SecretKeyEd25519,
        chain_id: &ChainId,
        shell_block_header: ShellBlockHeader,
        protocol_block_header: ProtocolBlockHeader,
        operations: Vec<serde_json::Value>,
    ) -> Result<BlockHash, TezosClientError> {
        let ShellBlockHeader {
            level,
            proto,
            predecessor,
            timestamp,
            validation_pass,
            operations_hash,
            fitness,
            context,
        } = shell_block_header;
        let ProtocolBlockHeader {
            payload_hash,
            payload_round,
            proof_of_work_nonce,
            seed_nonce_hash,
            liquidity_baking_escape_vote,
            ..
        } = protocol_block_header;
        let full_block_header = FullHeader {
            level,
            proto,
            predecessor,
            timestamp: timestamp.parse::<DateTime<Utc>>().unwrap().timestamp().into(),
            validation_pass,
            operations_hash,
            fitness: fitness
                .into_iter()
                .map(|v| hex::decode(v).unwrap())
                .collect::<Vec<_>>()
                .into(),
            context,
            payload_hash,
            payload_round,
            proof_of_work_nonce: SizedBytes(proof_of_work_nonce.try_into().unwrap()),
            seed_nonce_hash,
            liquidity_baking_escape_vote,
            signature: Signature(vec![])
        };

        let (signed, _) = sign_any(secret_key, 0x11, chain_id, &full_block_header)
            .expect("successful encode");

        #[derive(Serialize)]
        struct BlockData {
            data: String,
            operations: Vec<Vec<DecodedOperation>>,
        }

        let block_data = BlockData {
            data: hex::encode(signed),
            operations: {
                operations
                    .into_iter()
                    .map(|mut v| {
                        let applied = v.as_object_mut().unwrap().remove("applied").unwrap();
                        serde_json::from_value(applied).unwrap()
                    })
                    .collect()
            },
        };

        let url = self
            .endpoint
            .join("injection/block")
            .expect("valid constant url");

        let counter = self.counter.get();
        self.counter.set(counter + 1);
        slog::info!(self.log, ">>>>{}: {}", counter, url);
        let body = serde_json::to_string(&block_data)?;
        slog::info!(self.log, "{}", body);
        let mut response = self.inner.post(url).body(body).send()?;
        let status = response.status();
        slog::info!(self.log, "<<<<{}: {}", counter, status);

        if status.is_success() {
            let result = serde_json::from_reader(response).map_err(Into::into);
            match &result {
                Ok(value) => slog::info!(self.log, "{}", serde_json::to_string(value)?),
                Err(err) => slog::error!(self.log, "{}", err),
            }
            result
        } else {
            let mut buf = [0; 0x1000];
            io::Read::read(&mut response, &mut buf)?;
            let s = str::from_utf8(&buf)?.trim_end_matches('\0');
            slog::info!(self.main_logger, "{status}, {s}");
            Err(TezosClientError::Custom(s.to_string()))
        }
    }

    pub fn preapply_block(
        &self,
        secret_key: &SecretKeyEd25519,
        chain_id: &ChainId,
        protocol_block_header: ProtocolBlockHeader,
        mut operations: [Vec<serde_json::Value>; 4],
        timestamp: String,
    ) -> Result<(ShellBlockHeader, Vec<serde_json::Value>), TezosClientError> {
        #[derive(Serialize)]
        struct BlockData {
            protocol_data: serde_json::Value,
            operations: [Vec<serde_json::Value>; 4],
        }

        #[derive(Serialize, Deserialize)]
        struct PreapplyResponse {
            shell_header: ShellBlockHeader,
            operations: Vec<serde_json::Value>,
        }

        let proof_of_work_str = hex::encode(&protocol_block_header.proof_of_work_nonce);
        let signature = protocol_block_header
            .sign(secret_key, chain_id)
            .expect("successful encode");
        let mut protocol_data = serde_json::to_value(&protocol_block_header)?;
        let protocol_block_header_obj = protocol_data
            .as_object_mut()
            .expect("`ProtocolBlockHeader` is a structure");
        protocol_block_header_obj.insert(
            "signature".to_string(),
            serde_json::Value::String(signature.to_base58_check().to_string()),
        );
        protocol_block_header_obj.insert(
            "proof_of_work_nonce".to_string(),
            serde_json::Value::String(proof_of_work_str),
        );

        for i in 0..4 {
            for op in &mut operations[i] {
                if let Some(op_obj) = op.as_object_mut() {
                    op_obj.remove("hash");
                }
            }
        }

        let block_data = BlockData {
            protocol_data,
            operations,
        };

        let mut url = self
            .endpoint
            .join("chains/main/blocks/head/helpers/preapply/block")
            .expect("valid constant url");
        url.query_pairs_mut().append_pair("timestamp", &timestamp);

        let counter = self.counter.get();
        self.counter.set(counter + 1);
        slog::info!(self.log, ">>>>{}: {}", counter, url);
        let body = serde_json::to_string(&block_data)?;
        slog::info!(self.log, "{}", body);
        let mut response = self.inner.post(url).body(body).send()?;
        let status = response.status();
        slog::info!(self.log, "<<<<{}: {}", counter, status);
        if status.is_success() {
            let result =
                serde_json::from_reader::<_, PreapplyResponse>(response).map_err(Into::into);
            match &result {
                Ok(value) => slog::info!(self.log, "{}", serde_json::to_string(value)?),
                Err(err) => slog::error!(self.log, "{}", err),
            }
            result.map(|r| (r.shell_header, r.operations))
        } else {
            let mut buf = [0; 0x1000];
            io::Read::read(&mut response, &mut buf)?;
            let s = str::from_utf8(&buf)?.trim_end_matches('\0');
            slog::info!(self.main_logger, "{status}, {s}");
            Err(TezosClientError::Custom(s.to_string()))
        }
    }

    pub fn inject_operation(
        &self,
        chain_id: &ChainId,
        op_hex: &str,
    ) -> Result<serde_json::Value, TezosClientError> {
        let mut url = self
            .endpoint
            .join("injection/operation")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("chain", &chain_id.to_base58_check());

        let counter = self.counter.get();
        self.counter.set(counter + 1);
        slog::info!(self.log, ">>>>{}: {}", counter, url);
        let body = format!("{:?}", op_hex);
        slog::info!(self.log, "{}", body);
        let response = self.inner.post(url).body(body).send()?;
        let status = response.status();
        slog::info!(self.log, "<<<<{}: {}", counter, status);
        let result = serde_json::from_reader(response).map_err(Into::into);
        if !status.is_success() {
            slog::error!(self.main_logger, "{status}, {:?}", result);
        }
        match &result {
            Ok(value) => slog::info!(self.log, "{}", serde_json::to_string(value)?),
            Err(err) => slog::error!(self.main_logger, "{status}, {err}"),
        }
        result
    }

    /// nothing to do until bootstrapped, so let's wait synchronously
    pub fn wait_bootstrapped(&self) -> Result<serde_json::Value, TezosClientError> {
        let url = self
            .endpoint
            .join("monitor/bootstrapped")
            .expect("valid constant url");
        self.wrap_single_response(url, None)
    }

    pub fn constants(&self) -> Result<Constants, TezosClientError> {
        let url = self
            .endpoint
            .join("chains/main/blocks/head/context/constants")
            .expect("valid constant url");
        self.wrap_single_response(url, None)
    }

    pub fn validators(&self, level: i32) -> Result<Vec<Validator>, TezosClientError> {
        let mut url = self
            .endpoint
            .join("chains/main/blocks/head/helpers/validators")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("level", &level.to_string());
        self.wrap_single_response(url, None)
    }

    pub fn baking_rights(
        &self,
        level: i32,
        delegate: &ContractTz1Hash,
    ) -> Result<Vec<BakingRights>, TezosClientError> {
        let mut url = self
            .endpoint
            .join("chains/main/blocks/head/helpers/baking_rights")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("level", &level.to_string())
            .append_pair("delegate", &delegate.to_base58_check());
        self.wrap_single_response(url, None)
    }

    pub fn chain_id(&self) -> Result<ChainId, TezosClientError> {
        let url = self
            .endpoint
            .join("chains/main/chain_id")
            .expect("valid constant url");
        self.wrap_single_response(url, None)
    }

    fn wrap_single_response<T>(
        &self,
        url: Url,
        timeout: Option<Duration>,
    ) -> Result<T, TezosClientError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let (response, counter, status) = self.request_inner(url, timeout)?;
        slog::info!(self.log, "<<<<{}: {}", counter, status);
        let value = serde_json::from_reader::<_, serde_json::Value>(response)?;
        slog::info!(self.log, "{}", value);
        if !status.is_success() {
            slog::error!(self.main_logger, "{status}, {value}");
        }
        serde_json::from_value(value).map_err(Into::into)
    }

    pub fn monitor_main_head(&self) -> Result<impl Iterator<Item = BlockHeader>, TezosClientError> {
        let mut url = self
            .endpoint
            .join("monitor/heads/main")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("next_protocol", Self::PROTOCOL);
        self.wrap_response(url, None)
    }

    pub fn monitor_operations(
        &self,
        timeout: Option<Duration>,
    ) -> Result<impl Iterator<Item = Vec<serde_json::Value>>, TezosClientError> {
        let mut url = self
            .endpoint
            .join("chains/main/mempool/monitor_operations")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("applied", "yes")
            .append_pair("refused", "no")
            .append_pair("outdated", "no")
            .append_pair("branch_refused", "no")
            .append_pair("branch_delayed", "yes");
        self.wrap_response(url, timeout)
    }

    fn wrap_response<T>(
        &self,
        url: Url,
        timeout: Option<Duration>,
    ) -> Result<impl Iterator<Item = T>, TezosClientError>
    where
        for<'de> T: Deserialize<'de>,
    {
        let (response, counter, status) = self.request_inner(url, timeout)?;
        let log = self.log.clone();
        let main_logger = self.main_logger.clone();
        let it = serde_json::Deserializer::from_reader(response)
            .into_iter::<serde_json::Value>()
            .filter_map(move |v| match v {
                Ok(value) => {
                    if let Some(arr) = value.as_array() {
                        if arr.is_empty() {
                            return None;
                        }
                    }
                    slog::info!(log, "<<<<{}: {}", counter, status);
                    slog::info!(log, "{}", value);
                    serde_json::from_value(value).ok()
                }
                Err(err) => {
                    slog::info!(log, "<<<<{}: {}", counter, status);
                    slog::error!(main_logger, "{status}, {err}");
                    None
                }
            });
        Ok(it)
    }
}
