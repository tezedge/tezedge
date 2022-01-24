// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{cell::Cell, sync::mpsc, thread};

use derive_more::From;
use reqwest::{
    blocking::{Client, Response},
    StatusCode, Url,
};
use serde::Deserialize;
use slog::Logger;
use thiserror::Error;

use crypto::hash::{ChainId, BlockHash, ContractTz1Hash};

#[derive(Debug, Error, From)]
pub enum TezosClientError {
    #[error("{_0}")]
    Reqwest(reqwest::Error),
    #[error("{_0}")]
    SerdeJson(serde_json::Error),
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
    time_log: Logger,
}

#[derive(Deserialize)]
pub struct Constants {
    pub consensus_committee_size: u32,
}

#[derive(Deserialize)]
pub struct BlockHeader {
    pub level: u32,
    pub hash: BlockHash,
    pub predecessor: BlockHash,
    pub protocol_data: String,
}

#[derive(Deserialize)]
pub struct Validator {
    pub level: u32,
    pub delegate: ContractTz1Hash,
    pub slots: Vec<u16>,
}

impl TezosClient {
    // 012_PsiThaCa
    const PROTOCOL: &'static str = "PsiThaCaT47Zboaw71QWScM8sXeMM7bbQFncK9FLqYc6EKdpjVP";

    pub fn new(log: Logger, endpoint: Url) -> (Self, mpsc::Receiver<TezosClientEvent>) {
        let (tx, rx) = mpsc::channel();
        (
            TezosClient {
                tx,
                endpoint,
                inner: Client::new(),
                counter: Cell::new(0),
                log,
                time_log: crate::logger::logger_time(),
            },
            rx,
        )
    }

    fn request_inner(&self, url: Url) -> reqwest::Result<(Response, usize, StatusCode)> {
        let counter = self.counter.get();
        self.counter.set(counter + 1);
        slog::info!(self.log, ">>>>{}: {}", counter, url);
        let response = self.inner.get(url).send()?;
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
        let (response, counter, status) = self.request_inner(url)?;

        let mut deserializer =
            serde_json::Deserializer::from_reader(response).into_iter::<serde_json::Value>();

        let log = self.log.clone();
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
                            slog::error!(log, "{}", err);
                        }
                    }
                }
            })
            .expect("valid thread name");
        Ok(handle)
    }

    pub fn inject_operation(&self, chain_id: &ChainId, op_hex: &str) -> Result<serde_json::Value, TezosClientError> {
        let mut url = self.endpoint
            .join("injection/operation")
            .expect("valid constant url");
        url.query_pairs_mut().append_pair("chain", &chain_id.to_base58_check());

        let counter = self.counter.get();
        self.counter.set(counter + 1);
        slog::info!(self.log, ">>>>{}: {}", counter, url);
        slog::info!(self.time_log, "");
        let body = serde_json::to_string(op_hex).unwrap();
        slog::info!(self.log, "{}", body);
        let response = self.inner.post(url).body(body).send()?;
        let status = response.status();
        slog::info!(self.log, "<<<<{}: {}", counter, status);
        let result = serde_json::from_reader(response)
            .map_err(Into::into);
        match &result {
            Ok(value) => slog::info!(self.log, "{}", serde_json::to_string(value).unwrap()),
            Err(err) => slog::error!(self.log, "{}", err),
        }
        result
    }

    /// nothing to do until bootstrapped, so let's wait synchronously
    pub fn wait_bootstrapped(&self) -> Result<serde_json::Value, TezosClientError> {
        let url = self
            .endpoint
            .join("monitor/bootstrapped")
            .expect("valid constant url");
        self.wrap_single_response(url)
    }

    pub fn constants(&self) -> Result<Constants, TezosClientError> {
        let url = self.endpoint.join("chains/main/blocks/head/context/constants")
            .expect("valid constant url");
        self.wrap_single_response(url)
    }

    pub fn validators(&self, level: u32) -> Result<Vec<Validator>, TezosClientError> {
        let mut url = self.endpoint.join("chains/main/blocks/head/helpers/validators")
            .expect("valid constant url");
        url.query_pairs_mut().append_pair("level", &level.to_string());
        self.wrap_single_response(url)
    }

    pub fn chain_id(&self) -> Result<ChainId, TezosClientError> {
        let url = self.endpoint.join("chains/main/chain_id")
            .expect("valid constant url");
        self.wrap_single_response(url)
    }

    fn wrap_single_response<T>(&self, url: Url) -> Result<T, TezosClientError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let (response, counter, status) = self.request_inner(url)?;
        slog::info!(self.log, "<<<<{}: {}", counter, status);
        let value = serde_json::from_reader::<_, serde_json::Value>(response)?;
        slog::info!(self.log, "{}", value);
        serde_json::from_value(value).map_err(Into::into)
    }

    pub fn monitor_main_head(
        &self,
    ) -> Result<impl Iterator<Item = BlockHeader>, TezosClientError> {
        let mut url = self
            .endpoint
            .join("monitor/heads/main")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("next_protocol", Self::PROTOCOL);
        self.wrap_response(url)
    }

    pub fn monitor_operations(
        &self,
    ) -> Result<impl Iterator<Item = serde_json::Value>, TezosClientError> {
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
        self.wrap_response(url)
    }

    fn wrap_response<T>(&self, url: Url) -> Result<impl Iterator<Item = T>, TezosClientError>
    where
        for<'de > T: Deserialize<'de>,
    {
        let (response, counter, status) = self.request_inner(url)?;
        let log = self.log.clone();
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
                    slog::error!(log, "{}", err);
                    None
                }
            });
        Ok(it)
    }
}
