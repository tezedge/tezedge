use failure::{bail, Error};

use crate::rpc::message::PeerAddress;

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

pub fn lookup_initial_peers(bootstrap_addresses: &[String]) -> Result<Vec<PeerAddress>, Error> {
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