// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(no_coverage)]

use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::{fmt, path::PathBuf, str::FromStr};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use tezos_messages::base::rpc_support::{RpcJsonMap, UniversalValue};

pub const INMEM: &str = "inmem";
pub const ONDISK: &str = "ondisk";

#[derive(PartialEq, Eq, Hash, Debug, Clone, EnumIter)]
pub enum SupportedContextKeyValueStore {
    InMem,
    OnDisk,
}

impl SupportedContextKeyValueStore {
    pub fn possible_values() -> Vec<&'static str> {
        let mut possible_values = Vec::new();
        for sp in SupportedContextKeyValueStore::iter() {
            possible_values.extend(sp.supported_values());
        }
        possible_values
    }

    fn supported_values(&self) -> Vec<&'static str> {
        match self {
            SupportedContextKeyValueStore::InMem => vec!["inmem"],
            SupportedContextKeyValueStore::OnDisk => vec!["ondisk"],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseKeyValueStoreBackendError(String);

impl FromStr for SupportedContextKeyValueStore {
    type Err = ParseKeyValueStoreBackendError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_ascii_lowercase();
        for sp in SupportedContextKeyValueStore::iter() {
            if sp.supported_values().contains(&s.as_str()) {
                return Ok(sp);
            }
        }

        Err(ParseKeyValueStoreBackendError(format!(
            "Invalid variant name: {}",
            s
        )))
    }
}

// Must be in sync with ffi_config.ml
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TezosContextIrminStorageConfiguration {
    pub data_dir: String,
}

// Must be in sync with ffi_config.ml
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TezosContextTezedgeOnDiskBackendOptions {
    pub base_path: String,
    pub startup_check: bool,
}

// Must be in sync with ffi_config.ml
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ContextKvStoreConfiguration {
    ReadOnlyIpc,
    InMem,
    OnDisk(TezosContextTezedgeOnDiskBackendOptions),
}

// Must be in sync with ffi_config.ml
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TezosContextTezEdgeStorageConfiguration {
    pub backend: ContextKvStoreConfiguration,
    pub ipc_socket_path: Option<String>,
}

// Must be in sync with ffi_config.ml
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum TezosContextStorageConfiguration {
    IrminOnly(TezosContextIrminStorageConfiguration),
    TezEdgeOnly(TezosContextTezEdgeStorageConfiguration),
    Both(
        TezosContextIrminStorageConfiguration,
        TezosContextTezEdgeStorageConfiguration,
    ),
}

impl TezosContextStorageConfiguration {
    /// Used to produce a configuration for the readonly protocol runners from the configuration used in the main protocol runner.
    ///
    /// If only Irmin is enabled, the resulting configuration is the same.
    /// If only TezEdge is enabled, the resulting configuration switches the backend to the readonly IPC implementation.
    /// If both Irmin and TezEdge are enabled, an only-Irmin configuration is enabled.
    pub fn readonly(&self) -> Self {
        match self {
            TezosContextStorageConfiguration::IrminOnly(_) => self.clone(),
            TezosContextStorageConfiguration::TezEdgeOnly(tezedge) => {
                TezosContextStorageConfiguration::TezEdgeOnly(
                    TezosContextTezEdgeStorageConfiguration {
                        backend: ContextKvStoreConfiguration::ReadOnlyIpc,
                        ..tezedge.clone()
                    },
                )
            }
            TezosContextStorageConfiguration::Both(_irmin, tezedge) => {
                TezosContextStorageConfiguration::TezEdgeOnly(
                    TezosContextTezEdgeStorageConfiguration {
                        backend: ContextKvStoreConfiguration::ReadOnlyIpc,
                        ..tezedge.clone()
                    },
                )
            }
        }
    }

    /// Returns a copy with the path adjusted to `data_dir`
    pub fn with_path(&self, data_dir: String) -> Self {
        // TODO: only adjust on-disk backends, return copy otherwise
        match self {
            TezosContextStorageConfiguration::IrminOnly(_) => {
                TezosContextStorageConfiguration::IrminOnly(TezosContextIrminStorageConfiguration {
                    data_dir,
                })
            }
            TezosContextStorageConfiguration::TezEdgeOnly(_) => {
                TezosContextStorageConfiguration::TezEdgeOnly(
                    TezosContextTezEdgeStorageConfiguration {
                        backend: ContextKvStoreConfiguration::OnDisk(
                            TezosContextTezedgeOnDiskBackendOptions {
                                base_path: data_dir,
                                startup_check: false,
                            },
                        ),
                        ipc_socket_path: None,
                    },
                )
            }
            TezosContextStorageConfiguration::Both(_irmin, _tezedge) => self.clone(),
        }
    }

    pub fn get_ipc_socket_path(&self) -> Option<String> {
        match self {
            TezosContextStorageConfiguration::IrminOnly(_) => None,
            TezosContextStorageConfiguration::TezEdgeOnly(tezedge) => {
                tezedge.ipc_socket_path.clone()
            }
            TezosContextStorageConfiguration::Both(_irmin, tezedge) => {
                tezedge.ipc_socket_path.clone()
            }
        }
    }

    pub fn tezedge_is_enabled(&self) -> bool {
        match self {
            TezosContextStorageConfiguration::IrminOnly(_) => false,
            TezosContextStorageConfiguration::TezEdgeOnly(_) => true,
            TezosContextStorageConfiguration::Both(_, _) => true,
        }
    }
}

// Must be in sync with ffi_config.ml
#[derive(Serialize, Deserialize, Debug)]
pub struct TezosContextConfiguration {
    pub storage: TezosContextStorageConfiguration,
    pub genesis: GenesisChain,
    // TODO: move protocol_overrides elsewhere, out of the context
    pub protocol_overrides: ProtocolOverrides,
    pub commit_genesis: bool,
    // TODO: move enable_testchain elsewhere, out of the context
    pub enable_testchain: bool,
    pub readonly: bool,
    pub sandbox_json_patch_context: Option<PatchContext>,
    pub context_stats_db_path: Option<PathBuf>,
}

/// Genesis block information structure
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct GenesisChain {
    pub time: String,
    pub block: String,
    pub protocol: String,
}

/// Voted protocol overrides
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ProtocolOverrides {
    pub user_activated_upgrades: Vec<(i32, String)>,
    pub user_activated_protocol_overrides: Vec<(String, String)>,
}

impl ProtocolOverrides {
    pub fn user_activated_upgrades_to_rpc_json(&self) -> Vec<RpcJsonMap> {
        self.user_activated_upgrades
            .iter()
            .map(|(level, protocol)| {
                let mut json = RpcJsonMap::new();
                json.insert("level", UniversalValue::num(*level));
                json.insert(
                    "replacement_protocol",
                    UniversalValue::string(protocol.to_string()),
                );
                json
            })
            .collect::<Vec<RpcJsonMap>>()
    }

    pub fn user_activated_protocol_overrides_to_rpc_json(&self) -> Vec<RpcJsonMap> {
        self.user_activated_protocol_overrides
            .iter()
            .map(|(replaced_protocol, replacement_protocol)| {
                let mut json = RpcJsonMap::new();
                json.insert(
                    "replaced_protocol",
                    UniversalValue::string(replaced_protocol.to_string()),
                );
                json.insert(
                    "replacement_protocol",
                    UniversalValue::string(replacement_protocol.to_string()),
                );
                json
            })
            .collect::<Vec<RpcJsonMap>>()
    }
}

/// Patch_context key json
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct PatchContext {
    pub key: String,
    pub json: String,
}

impl fmt::Debug for PatchContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(key: {}, json: {})", &self.key, &self.json)
    }
}

pub type ContextKey<'a> = [&'a str];
pub type ContextKeyOwned = Vec<String>;
pub type ContextValue = Vec<u8>;

/// Tree in String form needed for JSON RPCs
pub type StringDirectoryMap = BTreeMap<String, StringTreeObject>;

/// Tree in String form needed for JSON RPCs
#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringTreeObject {
    #[cfg(fuzzing)]
    Directory,
    #[cfg(not(fuzzing))]
    Directory(StringDirectoryMap),
    Blob(String),
    Null,
}

/// Marco that simplifies and unificates ContextKey creation
///
/// Common usage:
///
/// `context_key!("protocol")`
/// `context_key!("data/votes/listings")`
/// `context_key!("data/rolls/owner/snapshot/{}/{}", cycle, snapshot)`
/// `context_key!("{}/{}/{}", "data", "votes", "listings")`
///
#[macro_export]
macro_rules! context_key {
    ($key:expr) => {{
        $key.split('/').collect::<Vec<&str>>()

    }};
    ($($arg:tt)*) => {{
        context_key!(format!($($arg)*))
    }};
}

// Like [`context_key`] but produces ann owned key.
#[macro_export]
macro_rules! context_key_owned {
    ($key:expr) => {{
        $key.split('/').map(String::from).collect::<Vec<String>>()
    }};
    ($($arg:tt)*) => {{
        context_key_owned!(format!($($arg)*))
    }};
}
