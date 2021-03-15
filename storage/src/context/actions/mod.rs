// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::str::FromStr;

use failure::Fail;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use std::path::PathBuf;
use tezos_context::channel::ContextAction;

pub mod action_file;
pub mod action_file_storage;
pub mod context_action_storage;

pub const ROCKSDB: &str = "rocksdb";

#[derive(PartialEq, Debug, Clone, EnumIter)]
pub enum ContextActionStoreBackend {
    NoneBackend,
    RocksDB,
    FileStorage { path: PathBuf },
}

impl ContextActionStoreBackend {
    pub fn possible_values() -> Vec<&'static str> {
        let mut possible_values = Vec::new();
        for sp in ContextActionStoreBackend::iter() {
            possible_values.extend(sp.supported_values());
        }
        possible_values
    }

    pub fn supported_values(&self) -> Vec<&'static str> {
        match self {
            ContextActionStoreBackend::RocksDB => vec![ROCKSDB],
            ContextActionStoreBackend::FileStorage { .. } => vec!["file"],
            ContextActionStoreBackend::NoneBackend => vec!["none"],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseContextActionStoreBackendError(String);

impl FromStr for ContextActionStoreBackend {
    type Err = ParseContextActionStoreBackendError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_ascii_lowercase();
        for sp in ContextActionStoreBackend::iter() {
            if sp.supported_values().contains(&s.as_str()) {
                return Ok(sp);
            }
        }

        Err(ParseContextActionStoreBackendError(format!(
            "Invalid variant name: {}",
            s
        )))
    }
}

pub trait ActionRecorder {
    fn record(&mut self, action: &ContextAction) -> Result<(), ActionRecorderError>;
}

#[derive(Debug, Fail)]
pub enum ActionRecorderError {
    #[fail(display = "Failed to store action, reason: {}", reason)]
    StoreError { reason: String },
    #[fail(display = "Missing actions for block {}.", hash)]
    MissingActions { hash: String },
}
