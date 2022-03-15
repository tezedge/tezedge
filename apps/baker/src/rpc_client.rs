// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{io, str, sync::mpsc, thread, time::Duration, convert::TryInto};

use chrono::{DateTime, Utc};
use derive_more::From;
use reqwest::{
    blocking::{Client, Response},
    StatusCode, Url,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tezos_encoding::types::SizedBytes;
use slog::Logger;
use thiserror::Error;

use crypto::hash::{BlockHash, ChainId, ContractTz1Hash, OperationHash, Signature};
use tezos_messages::{
    p2p::encoding::operation::DecodedOperation,
    protocol::proto_012::operation::{FullHeader, Operation},
};

use super::types::ShellBlockHeader;
use crate::{
    machine::action::*,
    types::{
        BlockInfo, DelegateSlots, FullHeader as FullHeaderJson, Mempool, Proposal,
        ProtocolBlockHeader, ShellBlockShortHeader, Slots, Timestamp,
    },
};

#[derive(Clone)]
pub struct RpcClient {
    tx: mpsc::Sender<Action>,
    endpoint: Url,
    inner: Client,
    logger: Logger,
}

#[derive(Debug, Error, From)]
pub enum RpcError {
    #[error("reqwest: {_0}")]
    Reqwest(reqwest::Error),
    #[error("serde_json: {_0}")]
    SerdeJson(serde_json::Error),
    #[error("io: {_0}")]
    Io(io::Error),
    #[error("utf8: {_0}")]
    Utf8(str::Utf8Error),
    #[error("http: {_0}")]
    Http(StatusCode),
}

#[derive(Deserialize, Debug)]
pub struct Constants {
    pub consensus_committee_size: u32,
    pub minimal_block_delay: String,
    pub delay_increment_per_round: String,
    pub blocks_per_commitment: u32,
    pub proof_of_work_threshold: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Validator {
    pub level: i32,
    pub delegate: ContractTz1Hash,
    pub slots: Vec<u16>,
}

impl RpcClient {
    // 012-Psithaca
    pub const PROTOCOL: &'static str = "Psithaca2MLRFYargivpo7YvUr7wUDqyxrdhC5CQq78mRvimz6A";

    pub fn new(endpoint: Url, logger: Logger, tx: mpsc::Sender<Action>) -> Self {
        RpcClient {
            tx,
            endpoint,
            inner: Client::new(),
            logger,
        }
    }

    pub fn get_constants(&self) -> Result<Constants, RpcError> {
        let url = self
            .endpoint
            .join("chains/main/blocks/head/context/constants")
            .expect("valid constant url");
        self.single_response_blocking(url, None)
    }

    /// nothing to do until bootstrapped, so let's wait synchronously
    pub fn wait_bootstrapped(&self) -> Result<serde_json::Value, RpcError> {
        let url = self
            .endpoint
            .join("monitor/bootstrapped")
            .expect("valid constant url");
        self.single_response_blocking(url, None)
    }

    pub fn get_chain_id(&self) -> Result<ChainId, RpcError> {
        let url = self
            .endpoint
            .join("chains/main/chain_id")
            .expect("valid constant url");
        self.single_response_blocking(url, None)
    }

    pub fn monitor_proposals<F, G>(
        &self,
        chain_id: &ChainId,
        this_delegate: ContractTz1Hash,
        deadline: i64,
        deadline_wrapper: G,
        wrapper: F,
    ) -> reqwest::Result<thread::JoinHandle<()>>
    where
        F: Fn(NewProposalAction) -> Action + Sync + Send + 'static,
        G: Fn(TimeoutAction) -> Action + Sync + Send + 'static,
    {
        let s = format!("monitor/heads/{chain_id}");
        let mut url = self.endpoint.join(&s).expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("next_protocol", Self::PROTOCOL);
        let this = self.clone();
        self.multiple_responses::<ShellBlockHeader, _, _>(
            url,
            deadline,
            deadline_wrapper,
            move |shell_header| {
                let hash = shell_header.hash.clone().to_base58_check();
                let predecessor_hash = shell_header.predecessor.to_base58_check();

                let s = format!("chains/main/blocks/{}/protocols", hash);
                let url = this.endpoint.join(&s).expect("valid url");
                let timeout = Self::deadline_to_duration(deadline)?;
                let protocols = this.single_response_blocking(url, Some(timeout))?;
                let s = format!("chains/main/blocks/{}/operations", hash);
                let url = this.endpoint.join(&s).expect("valid url");
                let timeout = Self::deadline_to_duration(deadline)?;
                let operations =
                    this.single_response_blocking::<Vec<Vec<Operation>>>(url, Some(timeout))?;
                let operations = [
                    operations.get(0).cloned().unwrap_or(vec![]),
                    operations.get(1).cloned().unwrap_or(vec![]),
                    operations.get(2).cloned().unwrap_or(vec![]),
                    operations.get(3).cloned().unwrap_or(vec![]),
                ];
                let block = BlockInfo::new(shell_header, protocols, operations);

                let s = format!("chains/main/blocks/{}/header", predecessor_hash);
                let url = this.endpoint.join(&s).expect("valid url");
                let timeout = Self::deadline_to_duration(deadline)?;
                let shell_header =
                    this.single_response_blocking::<FullHeaderJson>(url, Some(timeout))?;
                let s = format!("chains/main/blocks/{}/protocols", predecessor_hash);
                let url = this.endpoint.join(&s).expect("valid url");
                let timeout = Self::deadline_to_duration(deadline)?;
                let protocols = this.single_response_blocking(url, Some(timeout))?;
                let s = format!("chains/main/blocks/{}/operations", predecessor_hash);
                let url = this.endpoint.join(&s).expect("valid url");
                let timeout = Self::deadline_to_duration(deadline)?;
                let operations =
                    this.single_response_blocking::<Vec<Vec<Operation>>>(url, Some(timeout))?;
                let operations = [
                    operations.get(0).cloned().unwrap_or(vec![]),
                    operations.get(1).cloned().unwrap_or(vec![]),
                    operations.get(2).cloned().unwrap_or(vec![]),
                    operations.get(3).cloned().unwrap_or(vec![]),
                ];
                let predecessor =
                    BlockInfo::new_with_full_header(shell_header, protocols, operations);

                let s = format!("chains/main/blocks/{}/helpers/validators", hash);
                let mut url = this.endpoint.join(&s).expect("valid constant url");
                url.query_pairs_mut()
                    .append_pair("level", &block.level.to_string());
                let timeout = Self::deadline_to_duration(deadline)?;
                let validators =
                    this.single_response_blocking::<Vec<Validator>>(url, Some(timeout))?;
                let delegate_slots = {
                    let mut v = DelegateSlots::default();
                    for validator in validators {
                        let Validator {
                            delegate, slots, ..
                        } = validator;
                        if delegate.eq(&this_delegate) {
                            v.slot = slots.first().cloned();
                        }
                        v.level = block.level;
                        v.delegates.insert(delegate, Slots(slots));
                    }
                    v
                };
                let s = format!("chains/main/blocks/{}/helpers/validators", predecessor_hash);
                let mut url = this.endpoint.join(&s).expect("valid constant url");
                url.query_pairs_mut()
                    .append_pair("level", &(block.level + 1).to_string());
                let timeout = Self::deadline_to_duration(deadline)?;
                let validators =
                    this.single_response_blocking::<Vec<Validator>>(url, Some(timeout))?;
                let next_level_delegate_slots = {
                    let mut v = DelegateSlots::default();
                    for validator in validators {
                        let Validator {
                            delegate, slots, ..
                        } = validator;
                        if delegate.eq(&this_delegate) {
                            v.slot = slots.first().cloned();
                        }
                        v.level = block.level + 1;
                        v.delegates.insert(delegate, Slots(slots));
                    }
                    v
                };

                Ok(wrapper(NewProposalAction {
                    new_proposal: Proposal { block, predecessor },
                    delegate_slots,
                    next_level_delegate_slots,
                    now_timestamp: Timestamp::now(),
                }))
            },
        )
    }

    pub fn monitor_operations<F, G>(
        &self,
        deadline: i64,
        deadline_wrapper: G,
        wrapper: F,
    ) -> reqwest::Result<thread::JoinHandle<()>>
    where
        F: Fn(NewOperationSeenAction) -> Action + Sync + Send + 'static,
        G: Fn(TimeoutAction) -> Action + Sync + Send + 'static,
    {
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
        self.multiple_responses(url, deadline, deadline_wrapper, move |operations| {
            Ok(wrapper(NewOperationSeenAction { operations }))
        })
    }

    pub fn inject_operation<F, G>(
        &self,
        chain_id: &ChainId,
        op_hex: &str,
        deadline: i64,
        deadline_wrapper: G,
        wrapper: F,
    ) -> reqwest::Result<thread::JoinHandle<()>>
    where
        F: Fn(OperationHash) -> Action + Sync + Send + 'static,
        G: Fn(TimeoutAction) -> Action + Sync + Send + 'static,
    {
        let mut url = self
            .endpoint
            .join("injection/operation")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("chain", &chain_id.to_base58_check());
        let body = format!("{:?}", op_hex);
        self.single_response(
            url,
            Some(body),
            deadline,
            deadline_wrapper,
            move |operation_hash| wrapper(operation_hash),
        )
    }

    pub fn preapply_block<F, G>(
        &self,
        protocol_block_header: ProtocolBlockHeader,
        mempool: Mempool,
        predecessor: Option<BlockHash>,
        timestamp: i64,
        deadline: i64,
        deadline_wrapper: G,
        wrapper: F,
    ) -> Result<thread::JoinHandle<()>, RpcError>
    where
        F: Fn(FullHeader, Vec<Vec<DecodedOperation>>) -> Action + Sync + Send + 'static,
        G: Fn(TimeoutAction) -> Action + Sync + Send + 'static,
    {
        #[derive(Serialize)]
        struct BlockData {
            protocol_data: serde_json::Value,
            operations: [Vec<serde_json::Value>; 4],
        }

        #[derive(Serialize, Deserialize)]
        struct PreapplyResponse {
            shell_header: ShellBlockShortHeader,
            operations: Vec<serde_json::Value>,
        }

        let mut protocol_data = serde_json::to_value(&protocol_block_header)?;
        let protocol_block_header_obj = protocol_data
            .as_object_mut()
            .expect("`ProtocolBlockHeader` is a structure");
        let proof_of_work_str = hex::encode(&protocol_block_header.proof_of_work_nonce);
        protocol_block_header_obj.insert(
            "proof_of_work_nonce".to_string(),
            serde_json::Value::String(proof_of_work_str),
        );
        protocol_block_header_obj.insert(
            "protocol".to_string(),
            serde_json::Value::String(Self::PROTOCOL.to_string()),
        );

        let p = &mempool.payload;
        let mut operations = [
            mempool
                .consensus_payload
                .iter()
                .chain(mempool.preendorsement_consensus_payload.iter())
                .map(|op| serde_json::to_value(op).unwrap())
                .collect::<Vec<_>>(),
            p.votes_payload
                .iter()
                .map(|op| serde_json::to_value(op).unwrap())
                .collect(),
            p.anonymous_payload
                .iter()
                .map(|op| serde_json::to_value(op).unwrap())
                .collect(),
            p.managers_payload
                .iter()
                .map(|op| serde_json::to_value(op).unwrap())
                .collect(),
        ];
        for i in 0..4 {
            for op in &mut operations[i] {
                if let Some(op_obj) = op.as_object_mut() {
                    op_obj.remove("hash");
                    // if let Some(sig) = op_obj.get_mut("signature") {
                    //     if sig.is_null() {
                    //         op_obj.remove("signature");
                    //     }
                    // }
                }
            }
        }
        let block_data = BlockData {
            protocol_data,
            operations,
        };

        let s = if let Some(predecessor) = predecessor {
            format!("chains/main/blocks/{predecessor}/helpers/preapply/block")
        } else {
            "chains/main/blocks/head/helpers/preapply/block".to_string()
        };
        let mut url = self.endpoint.join(&s).expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("timestamp", &timestamp.to_string());
        let body = serde_json::to_string(&block_data)?;
        // TODO: remove temporal
        slog::info!(self.logger, "{}", body);
        let logger = self.logger.clone();
        self.single_response(url, Some(body), deadline, deadline_wrapper, move |v| {
            let PreapplyResponse {
                shell_header,
                operations,
            } = v;
            slog::debug!(logger, "{}", serde_json::to_string(&operations).unwrap());
            let ShellBlockShortHeader {
                level,
                proto,
                predecessor,
                timestamp,
                validation_pass,
                operations_hash,
                fitness,
                context,
            } = shell_header;
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
                signature: Signature(vec![0; 64]),
            };

            wrapper(
                full_block_header,
                operations
                    .into_iter()
                    .map(|mut v| {
                        let applied = v.as_object_mut().unwrap().remove("applied").unwrap();
                        serde_json::from_value(applied).unwrap()
                    })
                    .collect(),
            )
        })
        .map_err(Into::into)
    }

    pub fn inject_block<F, G>(
        &self,
        level: i32,
        round: i32,
        header: Vec<u8>, // serialized
        operations: Vec<Vec<DecodedOperation>>,
        deadline: i64,
        deadline_wrapper: G,
        wrapper: F,
    ) -> Result<thread::JoinHandle<()>, RpcError>
    where
        F: Fn(BlockHash) -> Action + Sync + Send + 'static,
        G: Fn(TimeoutAction) -> Action + Sync + Send + 'static,
    {
        #[derive(Serialize)]
        struct BlockData {
            data: String,
            operations: Vec<Vec<DecodedOperation>>,
        }

        let block_data = BlockData {
            data: hex::encode(header),
            operations,
        };

        let url = self
            .endpoint
            .join("injection/block")
            .expect("valid constant url");
        let body = serde_json::to_string(&block_data)?;

        let logger = self.logger.clone();
        self.single_response(url, Some(body), deadline, deadline_wrapper, move |hash| {
            slog::info!(
                logger,
                "injection/block for level: {level}, round: {round}, {hash}"
            );
            wrapper(hash)
        })
        .map_err(Into::into)
    }

    fn get(&self, url: Url, timeout: Option<Duration>) -> reqwest::Result<Response> {
        let request = self.inner.get(url);
        let request = if let Some(timeout) = timeout {
            request.timeout(timeout)
        } else {
            request
        };
        request.send()
    }

    fn post(&self, url: Url, body: String, timeout: Option<Duration>) -> reqwest::Result<Response> {
        let request = self.inner.post(url).body(body);
        let request = if let Some(timeout) = timeout {
            request.timeout(timeout)
        } else {
            request
        };
        request.send()
    }

    fn single_response_blocking<T>(
        &self,
        url: Url,
        timeout: Option<Duration>,
    ) -> Result<T, RpcError>
    where
        T: DeserializeOwned,
    {
        let mut retry = 3;
        let mut response = loop {
            match self.get(url.clone(), timeout) {
                Ok(v) => break v,
                Err(err) => {
                    if retry == 0 {
                        return Err(RpcError::Reqwest(err));
                    } else {
                        retry -= 1;
                    }
                }
            }
        };
        if response.status().is_success() {
            serde_json::from_reader::<_, T>(response).map_err(Into::into)
        } else {
            Self::read_error(&mut response)?;
            Err(RpcError::Http(response.status()))
        }
    }

    fn single_response<T, F, G>(
        &self,
        url: Url,
        body: Option<String>,
        deadline: i64,
        deadline_wrapper: G,
        wrapper: F,
    ) -> reqwest::Result<thread::JoinHandle<()>>
    where
        T: DeserializeOwned + Send + 'static,
        F: FnOnce(T) -> Action + Send + 'static,
        G: Fn(TimeoutAction) -> Action + Send + 'static,
    {
        let tx = self.tx.clone();
        let timeout = match Self::deadline_to_duration(deadline) {
            Ok(v) => v,
            Err(_) => {
                return Ok(thread::spawn(move || {
                    let now = Utc::now();
                    let action = TimeoutAction {
                        now_timestamp: Duration::from_secs(now.timestamp() as u64),
                    };
                    let _ = tx.send(deadline_wrapper(action));
                }));
            }
        };
        let mut response = match body {
            None => self.get(url.clone(), Some(timeout))?,
            Some(body) => self.post(url.clone(), body, Some(timeout))?,
        };

        let logger = self.logger.clone();
        let handle = thread::spawn(move || {
            if response.status().is_success() {
                match serde_json::from_reader::<_, T>(response) {
                    Ok(value) => {
                        let _ = tx.send(wrapper(value));
                    }
                    Err(err) if err.is_io() => {
                        let io_err = io::Error::from(err);
                        if io_err.kind() == io::ErrorKind::TimedOut {
                            let now = Utc::now();
                            let action = TimeoutAction {
                                now_timestamp: Duration::from_secs(now.timestamp() as u64),
                            };
                            let _ = tx.send(deadline_wrapper(action));
                        } else {
                            let action = UnrecoverableErrorAction {
                                description: "".to_string(),
                                rpc_error: io_err.into(),
                            };
                            let _ = tx.send(Action::UnrecoverableError(action));
                            panic!("{}", url)
                        }
                    }
                    Err(err) => {
                        let action = UnrecoverableErrorAction {
                            description: "".to_string(),
                            rpc_error: err.into(),
                        };
                        let _ = tx.send(Action::UnrecoverableError(action));
                        panic!("{}", url)
                    }
                }
            } else {
                let action = match Self::read_error(&mut response) {
                    Ok(error) => {
                        slog::error!(logger, "{}", error.description);
                        Action::RecoverableError(error)
                    }
                    Err(rpc_error) => Action::UnrecoverableError(UnrecoverableErrorAction {
                        description: "IO error while reading error".to_string(),
                        rpc_error,
                    }),
                };
                let _ = tx.send(action);
            }
        });
        Ok(handle)
    }

    fn multiple_responses<T, F, G>(
        &self,
        url: Url,
        deadline: i64,
        deadline_wrapper: G,
        wrapper: F,
    ) -> reqwest::Result<thread::JoinHandle<()>>
    where
        T: DeserializeOwned + Send + 'static,
        F: Fn(T) -> Result<Action, RpcError> + Send + 'static,
        G: Fn(TimeoutAction) -> Action + Send + 'static,
    {
        let tx = self.tx.clone();
        let timeout = match Self::deadline_to_duration(deadline) {
            Ok(v) => v,
            Err(_) => {
                return Ok(thread::spawn(move || {
                    let now = Utc::now();
                    let action = TimeoutAction {
                        now_timestamp: Duration::from_secs(now.timestamp() as u64),
                    };
                    let _ = tx.send(deadline_wrapper(action));
                }));
            }
        };
        let mut response = self.get(url.clone(), Some(timeout))?;

        let logger = self.logger.clone();
        let handle = thread::spawn(move || {
            let status = response.status();

            if status.is_success() {
                let mut deserializer =
                    serde_json::Deserializer::from_reader(response).into_iter::<T>();
                while let Some(v) = deserializer.next() {
                    match v.map_err(Into::into).and_then(|v| wrapper(v)) {
                        Ok(action) => {
                            let _ = tx.send(action);
                        }
                        Err(RpcError::Io(io_err)) if io_err.kind() == io::ErrorKind::TimedOut => {
                            let now = Utc::now();
                            let action = TimeoutAction {
                                now_timestamp: Duration::from_secs(now.timestamp() as u64),
                            };
                            let _ = tx.send(deadline_wrapper(action));
                            break;
                        }
                        Err(rpc_error) => {
                            let action = UnrecoverableErrorAction {
                                description: "".to_string(),
                                rpc_error,
                            };
                            let _ = tx.send(Action::UnrecoverableError(action));
                            // panic!("{}", url)
                        }
                    }
                }
            } else {
                let action = match Self::read_error(&mut response) {
                    Ok(error) => {
                        slog::error!(logger, "{}", error.description);
                        Action::RecoverableError(error)
                    }
                    Err(rpc_error) => Action::UnrecoverableError(UnrecoverableErrorAction {
                        description: "IO error while reading error".to_string(),
                        rpc_error,
                    }),
                };
                let _ = tx.send(action);
            }
        });
        Ok(handle)
    }

    // it may be string without quotes, it is invalid json, let's read it manually
    fn read_error(response: &mut impl io::Read) -> Result<RecoverableErrorAction, RpcError> {
        let mut buf = [0; 0x1000];
        io::Read::read(response, &mut buf)?;
        let err = str::from_utf8(&buf)?.trim_end_matches('\0');
        Ok(RecoverableErrorAction {
            description: err.to_string(),
        })
    }

    fn deadline_to_duration(deadline: i64) -> Result<Duration, RpcError> {
        let now = Utc::now();
        if let Some(deadline_millis) = deadline.checked_mul(1_000) {
            let millis = deadline_millis - now.timestamp_millis();
            if millis > 0 {
                Ok(Duration::from_millis(millis as u64))
            } else {
                Err(RpcError::Io(io::ErrorKind::TimedOut.into()))
            }
        } else {
            Ok(Duration::from_secs(2 << 21))
        }
    }
}
