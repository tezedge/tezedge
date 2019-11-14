// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use failure::Fail;

use tezos_api::identity::Identity;

const TEZOS_DIR: &str = ".tezos-node";
const IDENTITY_FILE: &str = "identity.json";

#[derive(Fail, Debug)]
pub enum IdentityError {
    #[fail(display = "Cannot determine path of the user home directory")]
    CannotFindHome,
    #[fail(display = "I/O error: {}", reason)]
    IoError {
        reason: io::Error
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

pub fn store_identity_to_default_tezos_identity_json_file(identity: &Identity) -> Result<(), IdentityError> {
    let tezos_home = dirs::home_dir()
        .map(|mut home_dir| {
            home_dir.push(TEZOS_DIR);
            home_dir
        })
        .ok_or(IdentityError::CannotFindHome)?;

    fs::create_dir_all(tezos_home)?;

    let identity_json = serde_json::to_string(identity).map_err(|err| IdentityError::SerializationError { reason: err })?;
    fs::write(&get_default_tezos_identity_json_file_path()?, &identity_json)?;

    Ok(())
}

pub fn get_default_tezos_identity_json_file_path() -> Result<PathBuf, IdentityError> {
    dirs::home_dir()
        .map(|mut home_dir| {
            home_dir.push(TEZOS_DIR);
            home_dir.push(IDENTITY_FILE);
            home_dir
        })
        .ok_or(IdentityError::CannotFindHome)
}
