// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{io, str, sync::mpsc, thread, time::Duration};

use derive_more::From;
use reqwest::{
    blocking::{Client, Response},
    Url,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

use crypto::hash::{BlockHash, ChainId, ContractTz1Hash, OperationHash};

use crate::machine::action::*;

pub struct RpcClient {
    tx: mpsc::Sender<Action>,
    endpoint: Url,
    inner: Client,
}

#[derive(Debug, Error, From)]
pub enum RpcError {
    #[error("{_0}")]
    Reqwest(reqwest::Error),
    #[error("{_0}")]
    SerdeJson(serde_json::Error),
    #[error("{_0}")]
    Io(io::Error),
    #[error("{_0}")]
    Utf8(str::Utf8Error),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BlockHeaderJson {
    pub level: i32,
    pub predecessor: BlockHash,
    pub timestamp: String,
    pub protocol_data: String,
    pub hash: BlockHash,
}

#[derive(Deserialize, Debug)]
pub struct Constants {
    pub consensus_committee_size: u32,
    pub minimal_block_delay: String,
    pub delay_increment_per_round: String,
}

#[derive(Deserialize, Debug)]
pub struct Validator {
    pub level: i32,
    pub delegate: ContractTz1Hash,
    pub slots: Vec<u16>,
}

impl RpcClient {
    // 012-Psithaca
    pub const PROTOCOL: &'static str = "Psithaca2MLRFYargivpo7YvUr7wUDqyxrdhC5CQq78mRvimz6A";

    pub fn new(endpoint: Url) -> (Self, impl Iterator<Item = Action>) {
        let (tx, rx) = mpsc::channel();
        (
            RpcClient {
                tx,
                endpoint,
                inner: Client::new(),
            },
            rx.into_iter(),
        )
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

    pub fn get_validators<F>(
        &self,
        level: i32,
        this_delegate: ContractTz1Hash,
        wrapper: F,
    ) -> reqwest::Result<thread::JoinHandle<()>>
    where
        F: Fn(GetSlotsSuccessAction) -> Action + Send + 'static,
    {
        let mut url = self
            .endpoint
            .join("chains/main/blocks/head/helpers/validators")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("level", &level.to_string());

        self.single_response::<Vec<Validator>, _>(url, None, None, move |validators| {
            wrapper(GetSlotsSuccessAction {
                validators,
                this_delegate,
            })
        })
    }

    pub fn monitor_main_head<F>(&self, wrapper: F) -> reqwest::Result<thread::JoinHandle<()>>
    where
        F: Fn(NewHeadSeenAction) -> Action + Sync + Send + 'static,
    {
        let mut url = self
            .endpoint
            .join("monitor/heads/main")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("next_protocol", Self::PROTOCOL);
        self.multiple_responses(url, None, move |head| wrapper(NewHeadSeenAction { head }))
    }

    pub fn monitor_operations<F>(&self, wrapper: F) -> reqwest::Result<thread::JoinHandle<()>>
    where
        F: Fn(NewOperationSeenAction) -> Action + Sync + Send + 'static,
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
        self.multiple_responses(url, None, move |operations| {
            wrapper(NewOperationSeenAction { operations })
        })
    }

    pub fn inject_operation<F>(
        &self,
        chain_id: &ChainId,
        op_hex: &str,
        wrapper: F,
    ) -> reqwest::Result<thread::JoinHandle<()>>
    where
        F: Fn(OperationHash) -> Action + Sync + Send + 'static,
    {
        let mut url = self
            .endpoint
            .join("injection/operation")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("chain", &chain_id.to_base58_check());
        let body = format!("{:?}", op_hex);
        self.single_response::<OperationHash, _>(url, Some(body), None, move |operation_hash| {
            wrapper(operation_hash)
        })
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
        let mut response = self.get(url, timeout)?;
        if response.status().is_success() {
            serde_json::from_reader::<_, T>(response).map_err(Into::into)
        } else {
            Self::read_error(&mut response)?;
            unreachable!()
        }
    }

    fn single_response<T, F>(
        &self,
        url: Url,
        body: Option<String>,
        timeout: Option<Duration>,
        wrapper: F,
    ) -> reqwest::Result<thread::JoinHandle<()>>
    where
        T: DeserializeOwned + Send + 'static,
        F: FnOnce(T) -> Action + Send + 'static,
    {
        let mut response = match body {
            None => self.get(url, timeout)?,
            Some(body) => self.post(url, body, timeout)?,
        };

        let tx = self.tx.clone();
        let handle = thread::spawn(move || {
            if response.status().is_success() {
                match serde_json::from_reader::<_, T>(response) {
                    Ok(value) => {
                        let _ = tx.send(wrapper(value));
                    }
                    Err(err) => {
                        let action = UnrecoverableErrorAction {
                            rpc_error: err.into(),
                        };
                        let _ = tx.send(Action::UnrecoverableError(action));
                    }
                }
            } else {
                let action = match Self::read_error(&mut response) {
                    Ok(error) => Action::RecoverableError(error),
                    Err(rpc_error) => {
                        Action::UnrecoverableError(UnrecoverableErrorAction { rpc_error })
                    }
                };
                let _ = tx.send(action);
            }
        });
        Ok(handle)
    }

    fn multiple_responses<T, F>(
        &self,
        url: Url,
        timeout: Option<Duration>,
        wrapper: F,
    ) -> reqwest::Result<thread::JoinHandle<()>>
    where
        T: DeserializeOwned + Send + 'static,
        F: Fn(T) -> Action + Send + 'static,
    {
        let mut response = self.get(url, timeout)?;

        let tx = self.tx.clone();
        let handle = thread::spawn(move || {
            let status = response.status();

            if status.is_success() {
                let mut deserializer =
                    serde_json::Deserializer::from_reader(response).into_iter::<T>();
                while let Some(v) = deserializer.next() {
                    match v {
                        Ok(value) => {
                            let _ = tx.send(wrapper(value));
                        }
                        Err(err) => {
                            let action = UnrecoverableErrorAction {
                                rpc_error: err.into(),
                            };
                            let _ = tx.send(Action::UnrecoverableError(action));
                            panic!();
                        }
                    }
                }
            } else {
                let action = match Self::read_error(&mut response) {
                    Ok(error) => Action::RecoverableError(error),
                    Err(rpc_error) => {
                        Action::UnrecoverableError(UnrecoverableErrorAction { rpc_error })
                    }
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
}
