// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    fs::OpenOptions,
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

use thiserror::Error;

enum LockFileStatus {
    Removed,
    SamePID,
}

#[derive(Debug, Error)]
pub enum LockDatabaseError {
    #[error("IOError {0}")]
    IOError(#[from] std::io::Error),
    #[error("Database already locked by {pid} ({path})")]
    AlreadyLocked {
        path: PathBuf,
        pid: String,
        self_pid: String,
    },
}

pub struct Lock {
    path: PathBuf,
}

// Number of instantiated `Lock`
static NACTIVE_LOCK: AtomicUsize = AtomicUsize::new(0);

impl Drop for Lock {
    fn drop(&mut self) {
        if NACTIVE_LOCK.fetch_sub(1, Ordering::SeqCst) > 0 {
            // Do not delete the file when there are other `Lock`
            return;
        }

        if let Err(e) = std::fs::remove_file(&self.path) {
            eprintln!("Failed to remove database lock file: {:?}", e);
        };
    }
}

impl Lock {
    pub fn try_lock(base_path: &str) -> Result<Self, LockDatabaseError> {
        std::fs::create_dir_all(&base_path)?;

        let path = PathBuf::from(base_path).join("lock");
        Self::create_lock_file(path.as_path())?;

        NACTIVE_LOCK.fetch_add(1, Ordering::SeqCst);

        Ok(Self { path })
    }

    fn create_lock_file(path: &Path) -> Result<(), LockDatabaseError> {
        loop {
            match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(mut lock_file) => {
                    // The file has been succesfully created, write our PID
                    let pid = process::id().to_string();

                    lock_file.write_all(pid.as_bytes())?;
                    lock_file.sync_all()?;
                    return Ok(());
                }
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    match Self::remove_zombie_lock(path)? {
                        LockFileStatus::SamePID => {
                            // Our process already locked the file
                            return Ok(());
                        }
                        LockFileStatus::Removed => {
                            // Re-run the loop to lock the file
                        }
                    }
                }
                Err(e) => return Err(e.into()),
            };
        }
    }

    /// Open the existing lock file, read the PID and remove the file
    /// if the PID is not alive
    fn remove_zombie_lock(path: &Path) -> Result<LockFileStatus, LockDatabaseError> {
        // At this point the file exists, we need to check if the PID it
        // contains is still alive

        let mut file = match OpenOptions::new()
            .read(true)
            .write(false)
            // Only `Self::create_lock_file` must create it, or we
            // can have race condition
            .create(false)
            .open(path)
        {
            Ok(file) => file,
            Err(e) => {
                return match e.kind() {
                    ErrorKind::NotFound => Ok(LockFileStatus::Removed), // Another process removed the file
                    _ => Err(e.into()),
                };
            }
        };

        // Read the pid from the file
        let mut pid = String::with_capacity(10);
        file.read_to_string(&mut pid)?;

        if pid.is_empty() {
            // Another process created the file but hasn't written its PID yet
            // Consider the file removed.
            return Ok(LockFileStatus::Removed);
        }

        if pid == std::process::id().to_string() {
            return Ok(LockFileStatus::SamePID);
        }

        if Self::is_pid_alive(&pid) {
            return Err(LockDatabaseError::AlreadyLocked {
                path: path.to_owned(),
                pid,
                self_pid: std::process::id().to_string(),
            });
        }

        // The PID belongs to a dead process, remove the lock file
        std::fs::remove_file(path)?;

        Ok(LockFileStatus::Removed)
    }

    #[cfg(target_os = "linux")]
    fn is_pid_alive(pid: &str) -> bool {
        let proc_dir = PathBuf::from("/proc");

        if !proc_dir.exists() {
            // If `/proc/` doesn't exist, consider the pid as alive
            return true;
        }

        proc_dir.join(pid).exists()
    }

    #[cfg(not(target_os = "linux"))]
    fn is_pid_alive(pid: &str) -> bool {
        // TODO: Implement on macos
        true
    }
}
