use std::sync::Arc;
use std::sync::RwLock;

use failure::{bail, Error};
use futures::lock::Mutex;
use log::debug;

use crypto::{
    crypto_box::*,
    nonce::*
};

use crate::configuration;
use crate::rpc::message::PeerAddress;
use crate::tezos::p2p::message::BinaryMessage;

use super::stream::*;

pub type PeerId = String;
pub type PublicKey = Vec<u8>;


/// Contains peer state information. Fox example nonce values - both local and remove.
#[derive(Debug)]
pub struct PeerState {
    pub nonce_local: Nonce,
    pub nonce_remote: Nonce,
}

impl PeerState {
    pub fn new(nonce_local: &Nonce, nonce_remote: &Nonce) -> PeerState {
        PeerState {
            nonce_local: nonce_local.clone(),
            nonce_remote: nonce_remote.clone(),
        }
    }

    pub fn increment_nonce_local(&mut self) {
        self.nonce_local = self.nonce_local.increment();
    }

    pub fn increment_nonce_remote(&mut self) {
        self.nonce_remote = self.nonce_remote.increment();
    }
}

/// P2pPeer represents live secured connection with remote peer
///
/// node communicates with these remote peers
pub struct P2pPeer {
    /// Peer ID is created as hex string representation of peer public key bytes.
    peer_id: PeerId,
    // Peer ip/port
    peer: PeerAddress,
    /// Peer public key.
    public_key: PublicKey,
    /// Precomputed key is created from merge of peer public key and our secret key.
    /// It's used to speedup of crypto operations.
    precomputed_key: PrecomputedKey,

    /// Contains peer state information. This is for internal use by `P2pPeer` only.
    peer_state: Arc<RwLock<PeerState>>,
    ///
    rx: Arc<Mutex<MessageReader>>,
    tx: Arc<Mutex<MessageWriter>>,
}

impl P2pPeer {
    pub fn new(peer_public_key: PublicKey, node_sk_as_hex_string: &str, peer: PeerAddress, peer_state: PeerState, rx: MessageReader, tx: MessageWriter) -> Self {

        let peer_id = hex::encode(&peer_public_key);
        let peer_pk_as_hex_string = &hex::encode(&peer_public_key);

        P2pPeer {
            peer_id,
            peer,
            public_key: peer_public_key,
            precomputed_key: precompute(
                peer_pk_as_hex_string,
                node_sk_as_hex_string,
            ).expect("Failed to create precomputed key"),
            peer_state: Arc::new(RwLock::new(peer_state)),
            rx: Arc::new(Mutex::new(rx)),
            tx: Arc::new(Mutex::new(tx)),
        }
    }

    pub fn get_peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    pub fn get_public_key(&self) -> &PublicKey {
        &self.public_key
    }

    pub fn get_peer(&self) -> &PeerAddress {
        &self.peer
    }

    pub async fn write_message<'a>(&'a self, message: &'a impl BinaryMessage) -> Result<(), Error> {
        let message_bytes = message.as_bytes()?;
        if configuration::ENV.log_messages_as_hex {
            debug!("Message to send to peer {} as hex (without length): \n{}", self.peer_id, hex::encode(&message_bytes));
        } else {
            debug!("Message to send to peer {} ...", self.peer_id);
        }

        // encrypt
        let message_encrypted = encrypt(
            &message_bytes,
            &self.peer_state.read().unwrap().nonce_local,
            &self.precomputed_key,
        )?;
        if configuration::ENV.log_messages_as_hex {
            debug!("Message (enc) to send to peer {} as hex (without length): \n{}", self.peer_id, hex::encode(&message_encrypted));
        }

        // increment nonce
        self.peer_state.write().unwrap().increment_nonce_local();

        // send
        let mut tx = self.tx.lock().await;
        if let Err(e) = tx.write_message(&message_encrypted).await {
            bail!("Failed to transfer encoding: {:?}", e);
        }

        Ok(())
    }

    pub async fn read_message(&self) -> Result<Vec<u8>, Error> {
        // read
        let mut rx = self.rx.lock().await;
        let message_encrypted = rx.read_message().await?;

        // increment nonce
        let remote_nonce_to_use = self.peer_state.read().unwrap().nonce_remote.clone();
        self.peer_state.write().unwrap().increment_nonce_remote();

        // decrypt
        match decrypt(message_encrypted.get_contents(), &remote_nonce_to_use, &self.precomputed_key) {
            | Ok(message) => {
                if configuration::ENV.log_messages_as_hex {
                    debug!("Message received from peer {} as hex: \n{}", self.peer_id, hex::encode(&message));
                } else {
                    debug!("Message received from peer {} ... ", self.peer_id);
                }
                Ok(message)
            }
            | Err(ref e) => {
                bail!("Decrypt encoding failed from peer: {} Reason: {:?}", self.peer_id, e)
            }
        }
    }
}