// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{cell::Cell, sync::mpsc, thread};

use derive_more::From;
use reqwest::{
    blocking::{Client, Response},
    StatusCode, Url,
};
use slog::Logger;
use thiserror::Error;

#[derive(Debug, Error, From)]
pub enum TezosClientError {
    #[error("{_0}")]
    Reqwest(reqwest::Error),
    #[error("{_0}")]
    SerdeJson(serde_json::Error),
}

#[derive(Debug)]
pub enum TezosClientEvent {
    Bootstrapped,
    NewHead(serde_json::Value),
    Operation(serde_json::Value),
}

pub struct TezosClient {
    tx: mpsc::Sender<TezosClientEvent>,
    endpoint: Url,
    inner: Client,
    counter: Cell<usize>,
    log: Logger,
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
            },
            rx,
        )
    }

    /// nothing to do until bootstrapped, so let's wait synchronously
    pub fn wait_bootstrapped(&self) -> Result<(), TezosClientError> {
        let url = self
            .endpoint
            .join("monitor/bootstrapped")
            .expect("valid constant url");
        let (response, counter, status) = self.request_inner(url)?;
        slog::info!(self.log, "<<<<{}: {}", counter, status);
        match serde_json::from_reader::<_, serde_json::Value>(response) {
            Ok(value) => slog::info!(self.log, "{}", value),
            Err(err) => slog::error!(self.log, "{}", err),
        }
        if let Err(_) = self.tx.send(TezosClientEvent::Bootstrapped) {
            slog::error!(self.log, "receiver is disconnected");
        }
        Ok(())
    }

    /// spawning a thread
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

    fn request_inner(&self, url: Url) -> reqwest::Result<(Response, usize, StatusCode)> {
        let counter = self.counter.get();
        self.counter.set(counter + 1);
        slog::info!(self.log, ">>>>{}: {}", counter, url);
        let response = self.inner.get(url).send()?;
        let status = response.status();
        Ok((response, counter, status))
    }

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
}
