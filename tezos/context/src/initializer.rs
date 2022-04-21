// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use ipc::IpcError;
use ocaml_interop::BoxRoot;
use parking_lot::RwLock;
pub use tezos_context_api::ContextKvStoreConfiguration;
use tezos_context_api::TezosContextTezEdgeStorageConfiguration;
use thiserror::Error;

use crate::kv_store::in_memory::{InMemory, InMemoryConfiguration};
use crate::kv_store::persistent::{Persistent, PersistentConfiguration};
use crate::kv_store::readonly_ipc::ReadonlyIpcBackend;
use crate::persistent::file::OpenFileError;
use crate::persistent::lock::LockDatabaseError;
use crate::serialize::DeserializationError;
use crate::{ContextKeyValueStore, PatchContextFunction, TezedgeContext, TezedgeIndex};

/// IPC communication errors
#[derive(Debug, Error)]
pub enum IndexInitializationError {
    #[error("Failure when initializing IPC context: {reason}")]
    IpcError {
        #[from]
        reason: IpcError,
    },
    #[error("Attempted to initialize an IPC context without a socket path")]
    IpcSocketPathMissing,
    #[error("Unexpected IO error occurred, {reason}")]
    IoError {
        #[from]
        reason: std::io::Error,
    },
    #[error("Deserialization error occured, {reason}")]
    DeserializationError {
        #[from]
        reason: DeserializationError,
    },
    #[error("Mutex/lock lock error! Reason: {reason}")]
    LockError { reason: String },
    #[error("Failed to open database file, {reason}")]
    OpenFileError {
        #[from]
        reason: OpenFileError,
    },
    #[error("Failed to lock database file, {reason}")]
    LockDatabaseError {
        #[from]
        reason: LockDatabaseError,
    },
    #[error("Sizes & checksums of the files do not match values in `sizes.db`")]
    InvalidIntegrity,
    #[error("Failed to join deserializing thread: {reason}")]
    ThreadJoinError { reason: String },
}

#[cfg(not(target_env = "msvc"))]
fn configure_jemalloc() -> tikv_jemalloc_ctl::Result<()> {
    use tikv_jemalloc_ctl::background_thread;

    // Enable `background_thread`, the jemalloc devs recommend to disable
    // them only on "esoteric situations"
    //
    // https://github.com/jemalloc/jemalloc/issues/956
    background_thread::write(true)?;
    let bg = background_thread::mib()?;
    log!("background_threads enabled: {}", bg.read()?);

    Ok(())
}

#[cfg(target_env = "msvc")]
fn configure_jemalloc() -> tikv_jemalloc_ctl::Result<()> {
    Ok(())
}

fn spawn_reload_database(
    repository: Arc<RwLock<ContextKeyValueStore>>,
) -> std::io::Result<JoinHandle<()>> {
    if let Err(e) = configure_jemalloc() {
        eprintln!("Failed to configure jemalloc: {:?}", e);
    };

    let thread = std::thread::Builder::new().name("db-reload".to_string());
    let (sender, recv) = std::sync::mpsc::channel();

    let result = thread.spawn(move || {
        let start_time = std::time::Instant::now();
        log!("Reloading context");

        let clone_repo = repository.clone();
        let mut repository = repository.write();

        // Notify the main thread that the repository is locked
        if let Err(e) = sender.send(()) {
            elog!("Failed to notify main thread that repo is locked: {:?}", e);
        }

        repository.store_own_repository(clone_repo);

        if let Err(e) = repository.reload_database() {
            elog!("Failed to reload repository: {:?}", e);
        }
        log!("Context reloaded in {:?}", start_time.elapsed());
    });

    // Wait for the spawned thread to lock the repository.
    // This is necessary to make sure that `TezedgeIndex` doesn't start
    // processing queries without having the repository synchronized
    // with data on disk
    if let Err(e) = recv.recv_timeout(Duration::from_secs(10)) {
        elog!("Failed to get notified that repo is locked: {:?}", e);
    }

    result
}

pub fn initialize_tezedge_index(
    configuration: &TezosContextTezEdgeStorageConfiguration,
    patch_context: Option<BoxRoot<PatchContextFunction>>,
) -> Result<TezedgeIndex, IndexInitializationError> {
    let repository: Arc<RwLock<ContextKeyValueStore>> = match configuration.backend {
        ContextKvStoreConfiguration::ReadOnlyIpc => match configuration.ipc_socket_path.clone() {
            None => return Err(IndexInitializationError::IpcSocketPathMissing),
            Some(ipc_socket_path) => Arc::new(RwLock::new(ReadonlyIpcBackend::try_connect(
                ipc_socket_path,
            )?)),
        },
        ContextKvStoreConfiguration::InMem(ref options) => {
            Arc::new(RwLock::new(InMemory::try_new(InMemoryConfiguration {
                db_path: Some(options.base_path.clone()),
                startup_check: options.startup_check,
            })?))
        }
        ContextKvStoreConfiguration::OnDisk(ref options) => {
            Arc::new(RwLock::new(Persistent::try_new(PersistentConfiguration {
                db_path: Some(options.base_path.clone()),
                startup_check: options.startup_check,
                read_mode: false,
            })?))
        }
    };

    // We reload the database in another thread, to avoid blocking on
    // `initialize_tezedge_index`: Reloading can take lots of time
    if let Err(e) = spawn_reload_database(Arc::clone(&repository)) {
        elog!("Failed to spawn thread to reload database: {:?}", e);
    }

    Ok(TezedgeIndex::new(repository, patch_context))
}

pub fn initialize_tezedge_context(
    configuration: &TezosContextTezEdgeStorageConfiguration,
) -> Result<TezedgeContext, IndexInitializationError> {
    let index = initialize_tezedge_index(configuration, None)?;
    Ok(TezedgeContext::new(index, None, None))
}
