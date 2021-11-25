// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    fs::OpenOptions,
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
    process,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LockDatabaseError {
    #[error("IOError {0}")]
    IOError(#[from] std::io::Error),
    #[error("Database already locked by {pid} ({path})")]
    AlreadyLocked { path: PathBuf, pid: String },
}

pub struct Lock {
    path: PathBuf,
}

impl Drop for Lock {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.path) {
            eprintln!("Failed to remove database lock file: {:?}", e);
        };
    }
}

impl Lock {
    pub fn try_lock(base_path: &str) -> Result<Self, LockDatabaseError> {
        std::fs::create_dir_all(&base_path)?;
        let path = PathBuf::from(base_path).join("lock");

        let mut lock_file = Self::create_lock_file(path.as_path())?;

        let pid = process::id().to_string();

        lock_file.write_all(pid.as_bytes())?;
        lock_file.sync_all()?;

        Ok(Self { path })
    }

    fn create_lock_file(path: &Path) -> Result<std::fs::File, LockDatabaseError> {
        loop {
            let file = OpenOptions::new().write(true).create_new(true).open(path);

            let error = match file {
                Ok(file) => return Ok(file),
                Err(e) => e,
            };

            match error.kind() {
                ErrorKind::AlreadyExists => {
                    Self::remove_zombie_lock(path)?;
                }
                _ => return Err(error.into()),
            }
        }
    }

    /// Open the existing lock file, read the PID and remove the file
    /// if the PID is not alive
    fn remove_zombie_lock(path: &Path) -> Result<(), LockDatabaseError> {
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
                    ErrorKind::NotFound => Ok(()), // Another process removed the file
                    _ => Err(e.into()),
                };
            }
        };

        // Read the pid from the file
        let mut pid = String::with_capacity(10);
        file.read_to_string(&mut pid)?;

        if pid.is_empty() {
            // Another process created the file but hasn't written its PID yet
            return Ok(());
        }

        if Self::is_pid_alive(&pid) {
            return Err(LockDatabaseError::AlreadyLocked {
                path: path.to_owned(),
                pid,
            });
        }

        // The PID belongs to a dead process, remove the lock file
        std::fs::remove_file(path)?;

        Ok(())
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
