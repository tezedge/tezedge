// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod client;
pub mod event;
pub mod key;
pub mod logger;
pub mod timer;

#[cfg(feature = "fuzzing")]
mod operation_mutator;

use std::{fs::File, path::Path, sync::mpsc, time::SystemTime};

use reqwest::Url;
use serde::{Deserialize, Serialize};

use redux_rs::TimeService;
use tenderbake as tb;

use crate::machine::BakerAction;

pub struct Services {
    pub client: client::RpcClient,
    pub crypto: key::CryptoService,
    pub log: slog::Logger,
    pub timer: timer::Timer,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EventWithTime {
    pub action: BakerAction,
    pub now: tenderbake::Timestamp,
}

impl Services {
    pub fn new(
        endpoint: Url,
        base_dir: &Path,
        baker: &str,
    ) -> (Self, impl Iterator<Item = EventWithTime>) {
        let (tx, rx) = mpsc::channel();

        (
            Self::new_with_id_and_log(endpoint, base_dir, baker, None, 0, tx),
            rx.into_iter().map(|(_, event)| {
                let unix_epoch = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap();
                let now = tb::Timestamp { unix_epoch };
                EventWithTime { now, action: event }
            }),
        )
    }

    pub fn new_with_id_and_log(
        endpoint: Url,
        base_dir: &Path,
        baker: &str,
        log: Option<File>,
        id: u8,
        tx: mpsc::Sender<(u8, BakerAction)>,
    ) -> Self {
        let log = match log {
            Some(f) => logger::file_logger(f),
            None => logger::main_logger(),
        };
        Services {
            client: client::RpcClient::new(endpoint, id, tx.clone()),
            crypto: key::CryptoService::read_key(&log, base_dir, baker).unwrap(),
            log,
            timer: timer::Timer::spawn(id, tx),
        }
    }
}

pub trait BakerService {
    fn client(&self) -> &client::RpcClient;

    fn crypto(&mut self) -> &mut key::CryptoService;

    fn log(&self) -> &slog::Logger;

    fn timer(&self) -> &timer::Timer;
}

impl BakerService for Services {
    fn client(&self) -> &client::RpcClient {
        &self.client
    }

    fn crypto(&mut self) -> &mut key::CryptoService {
        &mut self.crypto
    }

    fn log(&self) -> &slog::Logger {
        &self.log
    }

    fn timer(&self) -> &timer::Timer {
        &self.timer
    }
}

impl TimeService for Services {}
