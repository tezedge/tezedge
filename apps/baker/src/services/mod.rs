// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod client;
pub mod event;
pub mod key;
pub mod logger;
pub mod timer;

use std::{sync::mpsc, path::PathBuf};

use reqwest::Url;

pub struct Services {
    pub client: client::RpcClient,
    pub crypto: key::CryptoService,
    pub log: slog::Logger,
    pub timer: timer::Timer,
}

impl Services {
    pub fn new(endpoint: Url, base_dir: &PathBuf, baker: &str) -> (Self, impl Iterator<Item = Result<event::Event, client::RpcError>>) {
        let (tx, rx) = mpsc::channel();
        (
            Services {
                client: client::RpcClient::new(endpoint, tx.clone()),
                crypto: key::CryptoService::read_key(base_dir, baker).unwrap(),
                log: logger::main_logger(),
                timer: timer::Timer::spawn(tx),
            },
            rx.into_iter(),
        )
    }
}
