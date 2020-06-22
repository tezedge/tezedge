// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use failure::Fail;
use slog::{info, Logger};

use tezos_api::identity::Identity;
use tezos_client::client;

#[derive(Fail, Debug)]
pub enum IdentityError {
    #[fail(display = "I/O error: {}", reason)]
    IoError {
        reason: io::Error
    },
    #[fail(display = "Service error: {}, e: {}", message, error)]
    ServiceError {
        error: failure::Error,
        message: &'static str,
    },
    #[fail(display = "Identity serialization error: {}", reason)]
    SerializationError {
        reason: serde_json::Error
    },
    #[fail(display = "Identity de-serialization error: {}", reason)]
    DeserializationError {
        reason: serde_json::Error
    },

}

impl From<io::Error> for IdentityError {
    fn from(reason: io::Error) -> Self {
        IdentityError::IoError { reason }
    }
}

impl slog::Value for IdentityError {
    fn serialize(&self, _record: &slog::Record, key: slog::Key, serializer: &mut dyn slog::Serializer) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

/// Load identity from tezos configuration file.
pub fn load_identity<P: AsRef<Path>>(identity_json_file_path: P) -> Result<Identity, IdentityError> {
    let identity = fs::read_to_string(identity_json_file_path)
        .map(|contents| serde_json::from_str::<Identity>(&contents).map_err(|err| IdentityError::DeserializationError { reason: err }))??;
    Ok(identity)
}

/// Stores provided identity into the file specified by path
pub fn store_identity(path: &PathBuf, identity: &Identity) -> Result<(), IdentityError> {
    let identity_json = serde_json::to_string(identity).map_err(|err| IdentityError::SerializationError { reason: err })?;
    fs::write(&path, &identity_json)?;

    Ok(())
}

/// Ensures (load or create) identity exists according to the configuration
pub fn ensure_identity(identity_cfg: &crate::configuration::Identity, log: Logger) -> Result<Identity, IdentityError> {
    if identity_cfg.identity_json_file_path.exists() {
        load_identity(&identity_cfg.identity_json_file_path)
    } else {
        info!(log, "Generating new tezos identity. This will take a while"; "expected_pow" => identity_cfg.expected_pow);

        // TODO: TE-74 will be replace with rust version without protocol_runner
        match client::generate_identity(identity_cfg.expected_pow) {
            Ok(identity) => {
                info!(log, "Identity successfully generated");
                match store_identity(&identity_cfg.identity_json_file_path, &identity) {
                    Ok(()) => {
                        info!(log, "Generated identity stored to file"; "file" => identity_cfg.identity_json_file_path.clone().into_os_string().into_string().unwrap());
                        Ok(identity)
                    }
                    Err(e) => Err(e)
                }
            }
            Err(e) => return Err(IdentityError::ServiceError {
                error: e.into(),
                message: "Failed to generate identity",
            })
        }
    }
}