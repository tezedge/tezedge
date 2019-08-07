use std::fs;
use std::path::PathBuf;

use failure::{bail, Error};
use log::warn;

use crate::rpc::message::PeerAddress;
use crate::tezos::p2p::client::{Identity, Version};

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

pub fn load_identity(identity_json_file_path: PathBuf) -> Result<Identity, Error> {
    warn!("Using Tezos identity from file: {:?}", &identity_json_file_path);
    let contents = fs::read_to_string(&identity_json_file_path)?;
    Ok(serde_json::from_str::<Identity>(&contents)?)
}

pub fn versions() -> Vec<Version> {
    vec![Version { name: String::from("TEZOS_ALPHANET_2018-11-30T15:30:56Z"), major: 0, minor: 0 }]
}



pub fn genesis_chain_id() -> String {
    // TODO: real structure according to tezos-node and encoding [NetXgtSLGNJvNye]
    String::from("8eceda2f")
}