// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::fs;
use std::path::PathBuf;

use failure::{bail, Error};

use tezos_api::identity::Identity;

/// Load identity from tezos configuration file.
pub fn load_identity(identity_json_file_path: PathBuf) -> Result<Identity, Error> {
    let contents = fs::read_to_string(&identity_json_file_path)?;
    Ok(serde_json::from_str::<Identity>(&contents)?)
}

pub fn get_default_tezos_identity_json_file_path() -> Result<PathBuf, Error> {
    match dirs::home_dir() {
        Some(home_dir) => {
            let mut config_path = home_dir;
            config_path.push(".tezos-node");
            config_path.push("identity.json");
            Ok(config_path)
        }
        None => bail!("HOME directory not found"),
    }
}
