use std::{
    cell::{Ref, RefCell, RefMut},
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
};

use crate::transaction::transaction::{TransactionStatus, Tx};
use crypto::{
    crypto_box::{PrecomputedKey, PublicKey},
    nonce::NoncePair,
};
use std::net::SocketAddr;
use tezos_identity::Identity;
use tezos_messages::p2p::encoding::version::NetworkVersion;

pub struct Config {
    pub identity: Identity,
    pub network: NetworkVersion,
    pub pow_target: f64,
    pub max_peer_threshold: usize,
    pub disable_mempool: bool,
    pub private_node: bool,
}

pub struct ImmutableState {
    pub config: Config,
}

pub struct Peer {
    pub token: mio::Token,
    pub pub_key: PublicKey,
    pub nonce_pair: NoncePair,
    pub precomputed_key: PrecomputedKey,
}

// TODO: use nightly feature `adt_const_params`
pub const FIELD_CONNECTED_PEERS: u64 = 0;
pub const FIELD_GRAYLIST: u64 = 1;
pub const FIELD_FAILED_PEERS: u64 = 2;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum SharedStateField {
    ConnectedPeers,
    Graylist,
    FailedPeers,
    // rest of global state...
}

impl From<SharedStateField> for u64 {
    fn from(field: SharedStateField) -> Self {
        match field {
            SharedStateField::ConnectedPeers => FIELD_CONNECTED_PEERS,
            SharedStateField::Graylist => FIELD_GRAYLIST,
            SharedStateField::FailedPeers => FIELD_FAILED_PEERS,
        }
    }
}

impl From<u64> for SharedStateField {
    fn from(value: u64) -> Self {
        match value {
            FIELD_CONNECTED_PEERS => SharedStateField::ConnectedPeers,
            FIELD_GRAYLIST => SharedStateField::Graylist,
            FIELD_FAILED_PEERS => SharedStateField::FailedPeers,
            _ => unreachable!(),
        }
    }
}

/*
    We use two sets of shared-state access fields that transaction can perform:
    - read access set: can happen at any point during the transaction
    - write access set: can only happen from the transaction's `commit_reducer`
*/
#[derive(Clone)]
pub struct SharedStateAccess {
    read: Vec<SharedStateField>,
    write: Vec<SharedStateField>,
}

#[derive(Debug, Clone)]
pub enum SharedStateAccessError {
    ReadAccessDenied,
    WriteAccessDenied,
}

impl SharedStateAccess {
    pub fn new(read: &[SharedStateField], write: &[SharedStateField]) -> SharedStateAccess {
        SharedStateAccess {
            read: Vec::from(read),
            write: Vec::from(write),
        }
    }

    // returns true if two `SharedStateAccess` are mutually exclusive
    pub fn exclusive(&self, other: &Self) -> bool {
        self.read_exclusive(&other.read) || self.write_exclusive(&other.write)
    }

    pub fn in_read_set(&self, field: SharedStateField) -> bool {
        self.read.iter().any(|acc| *acc == field)
    }

    pub fn in_write_set(&self, field: SharedStateField) -> bool {
        self.write.iter().any(|acc| *acc == field)
    }

    // returns true if any of the `read_fields` is in the write set (of another transaction)
    fn read_exclusive(&self, read_fields: &Vec<SharedStateField>) -> bool {
        read_fields.iter().any(|field| self.in_write_set(*field))
    }

    // returns true if any of the `write_fields` is in the read set (of another transaction)
    fn write_exclusive(&self, write_fields: &Vec<SharedStateField>) -> bool {
        write_fields.iter().any(|field| self.in_read_set(*field))
    }
}

pub struct SharedField<T, const F: u64> {
    inner: T,
}

impl<T, const F: u64> SharedField<T, F> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    fn access(&self) -> SharedStateField {
        let acc = F;
        acc.into()
    }

    fn is_access_read(&self, tx: &Tx) -> bool {
        tx.access().in_read_set(self.access())
    }

    fn is_access_write(&self, tx: &Tx) -> bool {
        // to preserve atomicity only completed transactions can change shared-state
        tx.is_status(TransactionStatus::Completed) && tx.access().in_write_set(self.access())
    }

    pub fn get(&self, tx: &Tx) -> Result<&T, SharedStateAccessError> {
        match self.is_access_read(tx) {
            true => Ok(&self.inner),
            false => Err(SharedStateAccessError::ReadAccessDenied),
        }
    }

    pub fn get_mut(&mut self, tx: &Tx) -> Result<&mut T, SharedStateAccessError> {
        match self.is_access_write(tx) {
            true => Ok(&mut self.inner),
            false => Err(SharedStateAccessError::WriteAccessDenied),
        }
    }
}

type ShConnectedPeers = SharedField<BTreeMap<SocketAddr, Peer>, FIELD_CONNECTED_PEERS>;
type ShGraylist = SharedField<BTreeSet<IpAddr>, FIELD_GRAYLIST>;
type ShFailedPeers = SharedField<BTreeSet<mio::Token>, FIELD_FAILED_PEERS>;

pub struct SharedState {
    pub connected_peers: ShConnectedPeers,
    pub graylist: ShGraylist,
    pub failed_peers: ShFailedPeers, // rest of shared state
                                     // ...
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            connected_peers: ShConnectedPeers::new(BTreeMap::new()),
            graylist: ShGraylist::new(BTreeSet::new()),
            failed_peers: ShFailedPeers::new(BTreeSet::new()),
        }
    }
}

// state machine "state" (should be mutable only by reducers)
pub struct GlobalState {
    transaction_count: u64,
    // each transaction has its own local-state
    transactions: RefCell<BTreeMap<u64, Tx>>,
    // shared state (mutally-exclusive access permissions)
    sh_state: RefCell<SharedState>,
    // immutable state (read-only, can be accessed directly)
    im_state: ImmutableState,
}

impl GlobalState {
    pub fn new(im_state: ImmutableState) -> Self {
        Self {
            transaction_count: 0,
            transactions: RefCell::new(BTreeMap::new()),
            sh_state: RefCell::new(SharedState::new()),
            im_state,
        }
    }

    pub fn transactions(&self) -> Ref<BTreeMap<u64, Tx>> {
        self.transactions.borrow()
    }

    pub fn transactions_mut(&self) -> RefMut<BTreeMap<u64, Tx>> {
        self.transactions.borrow_mut()
    }

    pub fn add_transaction(&mut self, tx: Tx) -> u64 {
        let tx_id = self.transaction_count;
        self.transactions.borrow_mut().insert(tx_id, tx);
        self.transaction_count += 1;
        tx_id
    }

    pub fn remove_transaction(&self, tx_id: u64) -> Tx {
        // TODO: proper error handling
        self.transactions.borrow_mut().remove(&tx_id).unwrap()
    }

    pub fn is_tx_exclusive(&self, access: &SharedStateAccess) -> bool {
        self.transactions
            .borrow()
            .iter()
            .any(|(_, tx)| access.exclusive(tx.access()))
    }

    // shared state, all fields need access checks
    pub fn shared(&self) -> Ref<SharedState> {
        self.sh_state.borrow()
    }

    pub fn shared_mut(&self) -> RefMut<SharedState> {
        self.sh_state.borrow_mut()
    }

    // Immutable state doesn't need any kind of access checks
    pub fn immutable(&self) -> &ImmutableState {
        &self.im_state
    }
}
