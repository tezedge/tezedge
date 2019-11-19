// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use failure::Fail;

use tezos_api::identity::Identity;

#[derive(Fail, Debug)]
pub enum IdentityError {
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

// Stores provided identity into the file specified by path
pub fn store_identity(path: &PathBuf, identity: &Identity) -> Result<(), IdentityError> {
    // Tries to create identity parent dir, if non-existing
    if let Some(identity_parent_dir) = path.parent() {
        if identity_parent_dir.exists() == false {
            fs::create_dir_all(identity_parent_dir)?;
        }
    }

    let identity_json = serde_json::to_string(identity).map_err(|err| IdentityError::SerializationError { reason: err })?;
    fs::write(&path, &identity_json)?;

    Ok(())
}