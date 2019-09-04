use std::fs;
use std::path::PathBuf;

use failure::{bail, Error};
use log::warn;

#[derive(Clone, Debug, Deserialize)]
pub struct Identity {
    pub peer_id: String,
    pub public_key: String,
    pub secret_key: String,
    pub proof_of_work_stamp: String,
}

/// Load identity from tezos configuration file.
pub fn load_identity(identity_json_file_path: PathBuf) -> Result<Identity, Error> {
    warn!("Using Tezos identity from file: {:?}", &identity_json_file_path);
    let contents = fs::read_to_string(&identity_json_file_path)?;
    Ok(serde_json::from_str::<Identity>(&contents)?)
}

fn get_default_tezos_identity_json_file_path() -> Result<PathBuf, Error> {
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
