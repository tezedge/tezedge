use std::fs;
use std::path::PathBuf;

use failure::{bail, Error};
use log::{warn};

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

pub fn lookup_initial_peers(bootstrap_addresses: & Vec<String>) -> Result<Vec<PeerAddress>, Error> {
    let mut initial_peers = vec![];
    for address in bootstrap_addresses {
        match resolve_initial_peers(&address) {
            Ok(peers) => {
                initial_peers.extend(peers)
            },
            Err(e) => {
                bail!("DNS lookup for address: {:?} error: {:?}", &address, e)
            }
        }
    }
    Ok(initial_peers)
}

fn resolve_initial_peers(address: &str) -> Result<Vec<PeerAddress>, Error> {
    match dns_lookup::getaddrinfo(Some(address), None, None) {
        Ok(lookup) => {
            Ok(lookup.filter(Result::is_ok)
                .map(Result::unwrap)
                .map(|info| info.sockaddr.ip())
                .map(|ip| PeerAddress { host: ip.to_string(), port: 9732 })
                .collect())
        }
        Err(e) => bail!("DNS lookup for address: {:?} error: {:?}", &address, e)
    }
}

pub fn genesis_chain_id() -> String {
    // TODO: real structure according to tezos-node and encoding [NetXgtSLGNJvNye]
    String::from("8eceda2f")
}