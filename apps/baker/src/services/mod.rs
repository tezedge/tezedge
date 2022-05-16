// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod client;
pub mod event;
pub mod key;
pub mod logger;
pub mod timer;

#[cfg(feature = "fuzzing")]
mod operation_mutator;

use std::{
    path::PathBuf,
    sync::mpsc,
    time::SystemTime,
};

use reqwest::Url;

use redux_rs::TimeService;
use tenderbake as tb;

use crate::machine::BakerAction;

pub struct Services {
    pub client: client::RpcClient,
    pub crypto: key::CryptoService,
    pub log: slog::Logger,
    pub timer: timer::Timer,
}

pub struct EventWithTime {
    pub action: BakerAction,
    pub now: tenderbake::Timestamp,
}

impl Services {
    pub fn new(
        endpoint: Url,
        base_dir: &PathBuf,
        baker: &str,
    ) -> (Self, impl Iterator<Item = EventWithTime>) {
        let (tx, rx) = mpsc::channel();

        let srv = Services {
            client: client::RpcClient::new(endpoint, tx.clone()),
            crypto: key::CryptoService::read_key(base_dir, baker).unwrap(),
            log: logger::main_logger(),
            timer: timer::Timer::spawn(tx),
        };

        slog::info!(
            srv.log,
            "crypto service ready: {}",
            srv.crypto.public_key_hash()
        );

        (
            srv,
            rx.into_iter().map(|event| {
                let unix_epoch = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap();
                let now = tb::Timestamp { unix_epoch };
                EventWithTime { now, action: event }
            }),
        )
    }
}

pub trait BakerService {
    fn client(&self) -> &client::RpcClient;
    
    fn crypto(&self) -> &key::CryptoService;

    fn log(&self) -> &slog::Logger;

    fn timer(&self) -> &timer::Timer;
}

impl BakerService for Services {
    fn client(&self) -> &client::RpcClient {
        &self.client
    }
    
    fn crypto(&self) -> &key::CryptoService {
        &self.crypto
    }

    fn log(&self) -> &slog::Logger {
        &self.log
    }

    fn timer(&self) -> &timer::Timer {
        &self.timer
    }
}

impl TimeService for Services {}
