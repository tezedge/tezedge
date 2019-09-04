use std::convert::TryFrom;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::sync::RwLock;

use failure::{bail, Error};
use log::{debug, error, info, warn};
use serde::Deserialize;
use tokio::net::TcpStream;

use crypto::nonce::{self, Nonce, NoncePair};
use storage::db::Db;

use crate::p2p::{
    encoding::prelude::*,
    message::{BinaryMessage, RawBinaryMessage, JsonMessage},
    peer::{P2pPeer, PeerState},
    stream::MessageStream,
};
use crate::rpc::message::PeerAddress;


#[derive(Clone)]
pub struct P2pClient {
    listener_port: u16,
    init_chain_id: Vec<u8>,
    identity: Identity,
    versions: Vec<Version>,
    db: Arc<RwLock<Db>>,
    log_messages: bool,
}

impl P2pClient {
    pub fn new(init_chain_id: Vec<u8>, identity: Identity, versions: Vec<Version>, db: Db, listener_port: u16, log_messages: bool) -> Self {
        info!("Configuration - Rust node p2p listening on port: {:?}", listener_port);
        P2pClient { listener_port, init_chain_id, identity, log_messages, versions, db: Arc::new(RwLock::new(db)) }
    }



    pub async fn handle_message<'a>(&'a self, peer: &'a P2pPeer, message: &'a Vec<u8>) -> Result<(), Error> {
        let response = PeerMessageResponse::from_bytes(message.clone())?;
        if self.log_messages {
            debug!("Response received from peer [as JSON]: \n{}", response.as_json()?);
        }


        for peer_message in response.get_messages() {
            match peer_message {
                PeerMessage::GetCurrentBranch(get_current_branch_message) => {
                    debug!("Received get_current_branch_message from peer: {:?} for chain_id: {:?}", &peer.get_peer_id(), hex::encode(get_current_branch_message.get_chain_id()));

                    // replay with our current branch
                    match self.get_current_branch() {
                        None => debug!("No current branch"),
                        Some(current_branch) => {
                            // send back our current_branch
                            let message = PeerMessage::CurrentBranch(CurrentBranchMessage::from_bytes(current_branch.as_bytes()?)?);
                            let message: PeerMessageResponse = message.into();
                            match peer.write_message(&message).await {
                                Ok(()) => debug!("current branch sent to peer: {:?}", &peer.get_peer_id()),
                                Err(e) => error!("Failed to write current branch encoding. {:?}", e),
                            }
                        }
                    }
                }
                PeerMessage::CurrentBranch(current_branch_message) => {
                    debug!("Received current_branch_message from peer: {:?} for chain_id: {:?}", &peer.get_peer_id(), hex::encode(current_branch_message.get_chain_id()));
                    debug!("Received current_branch_message from peer: {:?} for current_head.operations_hash: {:?}", &peer.get_peer_id(), hex::encode(current_branch_message.get_current_branch().get_current_head().get_operations_hash()));

                    // (Demo) store current_branch in db
                    self.db.write().unwrap().store_branch(peer.get_peer_id().clone(), current_branch_message.as_bytes()?);
                }
                _ => warn!("Received encoding (but not handled): {:?}", peer_message)
            }
        }

        Ok(())
    }

    pub async fn start_p2p_biznis<'a>(&'a self, peer: &'a P2pPeer) -> () {
        let get_current_branch_message = PeerMessage::GetCurrentBranch(GetCurrentBranchMessage::new(self.init_chain_id.clone()));
        let response: PeerMessageResponse = get_current_branch_message.into();
        match peer.write_message(&response).await {
            Ok(()) => debug!("Write success"),
            Err(e) => error!("Failed to write encoding. {:?}", e),
        }
    }

    pub fn get_current_branch(&self) -> Option<CurrentBranchMessage> {
        // (Demo) get current_branch from db
        let branches = (&self.db.read().unwrap()).get_branches();

        // filter with highest level and fitness
        let branches: Vec<CurrentBranchMessage> = branches.into_iter()
            .map(|bytes| CurrentBranchMessage::from_bytes(bytes).unwrap())
            .collect();

        let current_branch = branches.iter().fold(None, |max, x| match max {
            None => Some(x),
            Some(max) => {
                let max_head = max.get_current_branch().get_current_head();
                let x_head = x.get_current_branch().get_current_head();
                if max_head.get_level() < x_head.get_level() {
                    Some(x)
                } else if max_head.get_level() == x_head.get_level() {
                    // get last fitness https://tezos.gitlab.io/alphanet/whitedoc/proof_of_stake.html
                    let fit_zero = hex::decode("00").unwrap();
                    let fit_for_max = max_head.get_fitness().last().unwrap_or(&fit_zero);
                    let fit_for_x = x_head.get_fitness().last().unwrap_or(&fit_zero);

                    if fit_for_max < fit_for_x {
                        Some(x)
                    } else {
                        Some(max)
                    }
                } else {
                    Some(max)
                }
            }
        });

        match current_branch {
            None => None,
            Some(branch) => {
                Some(CurrentBranchMessage::from_bytes(branch.as_bytes().unwrap()).unwrap())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;






}
