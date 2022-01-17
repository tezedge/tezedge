use std::{
    cell::{Ref, RefCell, RefMut},
    cmp::Ordering,
    collections::{btree_map::Iter, BTreeMap, BTreeSet},
    net::IpAddr,
};

use crate::transaction::{
    chunk::{TxRecvChunk, TxSendChunk},
    data_transfer::{TxRecvData, TxSendData},
    handshake::{
        ack::{TxRecvAckMessage, TxSendAckMessage},
        connection::{TxRecvConnectionMessage, TxSendConnectionMessage},
        handshake::TxHandshake,
        metadata::{TxRecvMetadataMessage, TxSendMetadataMessage},
    },
    transaction::{ParentInfo, Transaction, Tx, TxOps},
};
use crypto::{crypto_box::PrecomputedKey, nonce::NoncePair};
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

#[derive(Clone)]
pub enum Peer {
    Connected(mio::Token, SocketAddr, PrecomputedKey, NoncePair),
    Graylisted(IpAddr),
}

/*
    TODO: we can avoid implementing Ord by using a HashSet instead of a BTreeSet, but for that we need
    a deterministic (as in getting same results from different program executions) RandomState
    implementation to replace the default one.
*/
impl Ord for Peer {
    fn cmp(&self, other: &Self) -> Ordering {
        match self {
            Peer::Connected(token, ..) => match other {
                Peer::Connected(other_token, ..) => token.cmp(other_token),
                _ => Ordering::Less,
            },
            Peer::Graylisted(addr) => match other {
                Peer::Graylisted(other_addr) => addr.cmp(other_addr),
                _ => Ordering::Greater,
            },
        }
    }
}

impl PartialOrd for Peer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Peer {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Peer::Connected(token, ..) => match other {
                Peer::Connected(other_token, ..) => token == other_token,
                _ => false,
            },
            Peer::Graylisted(addr) => match other {
                Peer::Graylisted(other_addr) => addr == other_addr,
                _ => false,
            },
        }
    }
}

impl Eq for Peer {}

/*
    Use a `u64` bitmask to represent shared-state fields, each field has an
    unique constant (of the form 1 << n) associated to it. This allow us to
    represent up to 64 fields in an efficient way.
*/
pub type FieldAccessMask = u64;
pub const PEERS_ACCESS_FIELD: u64 = 1 << 0;

/*
    We use two sets of shared-state access fields that transaction can perform:
    - read access set: can happen at any point during the transaction
    - write access set: can only happen from the transaction's `commit`
*/
#[derive(Clone, Copy, Default)]
pub struct FieldAccess {
    pub read: FieldAccessMask,
    pub write: FieldAccessMask,
}

impl FieldAccess {
    pub fn new(read: FieldAccessMask, write: FieldAccessMask) -> FieldAccess {
        FieldAccess { read, write }
    }

    pub fn exclusive(&self, other: &Self) -> bool {
        self.read_exclusive(other.read) || self.write_exclusive(other.write)
    }

    pub fn read_exclusive(&self, read: FieldAccessMask) -> bool {
        (self.write & read) != 0
    }

    pub fn write_exclusive(&self, write: FieldAccessMask) -> bool {
        ((self.read | self.write) & write) != 0
    }

    pub fn has_read_access(&self, field: u64) -> bool {
        (self.read & field) != 0
    }

    pub fn has_write_access(&self, field: u64) -> bool {
        (self.write & field) != 0
    }
}

pub struct SharedField<T, const F: u64> {
    inner: RefCell<T>,
}

impl<T, const F: u64> SharedField<T, F> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: RefCell::new(inner),
        }
    }

    pub fn borrow(&self, access: &FieldAccess) -> Ref<T> {
        assert!(access.has_read_access(F));
        self.inner.borrow()
    }

    // TODO: allow only from commit()
    pub fn borrow_mut(&self, access: &FieldAccess) -> RefMut<T> {
        assert!(access.has_write_access(F));
        self.inner.borrow_mut()
    }
}

pub struct SharedState {
    pub peers: SharedField<BTreeSet<Peer>, PEERS_ACCESS_FIELD>,
    // rest of shared state...
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            peers: SharedField::new(BTreeSet::new()),
        }
    }
}

type TransactionMap<T> = BTreeMap<u64, RefCell<Tx<T>>>;
// TODO: if the number of transaction types grows too big we might need some macro to avoid code duplication
pub struct Transactions {
    transaction_count: u64,
    send_data: TransactionMap<TxSendData>,
    send_chunk: TransactionMap<TxSendChunk>,
    send_connection_message: TransactionMap<TxSendConnectionMessage>,
    send_metadata_message: TransactionMap<TxSendMetadataMessage>,
    send_ack_message: TransactionMap<TxSendAckMessage>,
    recv_data: TransactionMap<TxRecvData>,
    recv_chunk: TransactionMap<TxRecvChunk>,
    recv_connection_message: TransactionMap<TxRecvConnectionMessage>,
    recv_metadata_message: TransactionMap<TxRecvMetadataMessage>,
    recv_ack_message: TransactionMap<TxRecvAckMessage>,
    handshake: TransactionMap<TxHandshake>,
}

impl Default for Transactions {
    fn default() -> Self {
        Self {
            transaction_count: 0,
            send_data: BTreeMap::new(),
            send_chunk: BTreeMap::new(),
            send_connection_message: BTreeMap::new(),
            send_metadata_message: BTreeMap::new(),
            send_ack_message: BTreeMap::new(),
            recv_data: BTreeMap::new(),
            recv_chunk: BTreeMap::new(),
            recv_connection_message: BTreeMap::new(),
            recv_metadata_message: BTreeMap::new(),
            recv_ack_message: BTreeMap::new(),
            handshake: BTreeMap::new(),
        }
    }
}

impl Transactions {
    pub fn transactions_with_parent(&self) -> BTreeMap<u64, Option<ParentInfo>> {
        BTreeMap::from_iter(
            [
                self.send_data
                    .iter()
                    .map(|(tx_id, tx)| (*tx_id, tx.borrow().parent()))
                    .collect::<Vec<(u64, Option<ParentInfo>)>>(),
                self.send_chunk
                    .iter()
                    .map(|(tx_id, tx)| (*tx_id, tx.borrow().parent()))
                    .collect(),
                self.send_connection_message
                    .iter()
                    .map(|(tx_id, tx)| (*tx_id, tx.borrow().parent()))
                    .collect(),
                self.send_metadata_message
                    .iter()
                    .map(|(tx_id, tx)| (*tx_id, tx.borrow().parent()))
                    .collect(),
                self.send_ack_message
                    .iter()
                    .map(|(tx_id, tx)| (*tx_id, tx.borrow().parent()))
                    .collect(),
                self.recv_data
                    .iter()
                    .map(|(tx_id, tx)| (*tx_id, tx.borrow().parent()))
                    .collect(),
                self.recv_chunk
                    .iter()
                    .map(|(tx_id, tx)| (*tx_id, tx.borrow().parent()))
                    .collect(),
                self.recv_connection_message
                    .iter()
                    .map(|(tx_id, tx)| (*tx_id, tx.borrow().parent()))
                    .collect(),
                self.recv_metadata_message
                    .iter()
                    .map(|(tx_id, tx)| (*tx_id, tx.borrow().parent()))
                    .collect(),
                self.recv_ack_message
                    .iter()
                    .map(|(tx_id, tx)| (*tx_id, tx.borrow().parent()))
                    .collect(),
                self.handshake
                    .iter()
                    .map(|(tx_id, tx)| (*tx_id, tx.borrow().parent()))
                    .collect(),
            ]
            .concat(),
        )
    }

    pub fn transactions_with_access(&self) -> BTreeMap<u64, FieldAccess> {
        BTreeMap::from_iter(
            [
                self.send_data
                    .iter()
                    .map(|(tx_id, tx)| {
                        let tx = tx.borrow();
                        (*tx_id, *tx.access())
                    })
                    .collect::<Vec<(u64, FieldAccess)>>(),
                self.send_chunk
                    .iter()
                    .map(|(tx_id, tx)| {
                        let tx = tx.borrow();
                        (*tx_id, *tx.access())
                    })
                    .collect(),
                self.send_connection_message
                    .iter()
                    .map(|(tx_id, tx)| {
                        let tx = tx.borrow();
                        (*tx_id, *tx.access())
                    })
                    .collect(),
                self.send_metadata_message
                    .iter()
                    .map(|(tx_id, tx)| {
                        let tx = tx.borrow();
                        (*tx_id, *tx.access())
                    })
                    .collect(),
                self.send_ack_message
                    .iter()
                    .map(|(tx_id, tx)| {
                        let tx = tx.borrow();
                        (*tx_id, *tx.access())
                    })
                    .collect(),
                self.recv_data
                    .iter()
                    .map(|(tx_id, tx)| {
                        let tx = tx.borrow();
                        (*tx_id, *tx.access())
                    })
                    .collect(),
                self.recv_chunk
                    .iter()
                    .map(|(tx_id, tx)| {
                        let tx = tx.borrow();
                        (*tx_id, *tx.access())
                    })
                    .collect(),
                self.recv_connection_message
                    .iter()
                    .map(|(tx_id, tx)| {
                        let tx = tx.borrow();
                        (*tx_id, *tx.access())
                    })
                    .collect(),
                self.recv_metadata_message
                    .iter()
                    .map(|(tx_id, tx)| {
                        let tx = tx.borrow();
                        (*tx_id, *tx.access())
                    })
                    .collect(),
                self.recv_ack_message
                    .iter()
                    .map(|(tx_id, tx)| {
                        let tx = tx.borrow();
                        (*tx_id, *tx.access())
                    })
                    .collect(),
                self.handshake
                    .iter()
                    .map(|(tx_id, tx)| {
                        let tx = tx.borrow();
                        (*tx_id, *tx.access())
                    })
                    .collect(),
            ]
            .concat(),
        )
    }

    pub fn transaction_access(&self, tx_id: u64) -> FieldAccess {
        *self
            .transactions_with_access()
            .get(&tx_id)
            .unwrap_or_else(|| panic!("Transaction not found"))
    }

    pub fn ancestors(&self, tx_id: u64) -> BTreeSet<u64> {
        let transactions_with_parent = self.transactions_with_parent();
        let mut ancestors: BTreeSet<u64> = BTreeSet::new();
        let mut parent = transactions_with_parent.get(&tx_id).unwrap().clone();

        while parent.is_some() {
            let tx_id = parent.unwrap().tx_id;

            ancestors.insert(tx_id);
            parent = transactions_with_parent.get(&tx_id).unwrap().clone();
        }

        ancestors
    }

    pub fn is_access_exclusive(
        &self,
        parent: Option<ParentInfo>,
        transaction_access: &FieldAccess,
    ) -> bool {
        let transactions_with_access = self.transactions_with_access();
        let mut read_access = 0;
        let mut write_access = 0;

        if let Some(ParentInfo { tx_id, .. }) = parent {
            let ancestors = self.ancestors(tx_id);
            /*
                Children transactions can read into an ancestor's write-field (children complete before parent's commit).
                It could be possible for children transactions to write to fields as long as they don't overlap with
                the ancestor's fields, however this would need "rollback" support for transaction cancelation, which
                might require a complex implementation. When children transactions commit, they usually just need to
                notify the parent transaction and don't perform any other side-effects, so we just deny children
                transaction that attempts to write to to shared-state.
            */
            assert!(transaction_access.write == 0);

            // filter out ancestors' access
            for (_, access) in transactions_with_access
                .iter()
                .filter(|(tx_id, _)| ancestors.get(tx_id).is_none())
            {
                read_access |= access.read;
                write_access |= access.write;
            }
        } else {
            for (_, access) in transactions_with_access.iter() {
                read_access |= access.read;
                write_access |= access.write;
            }
        }

        transaction_access.exclusive(&FieldAccess::new(read_access, write_access))
    }
}

pub trait TransactionsOps<T: Transaction>
where
    Transactions: TransactionsOps<T>,
{
    fn insert(&mut self, tx: Tx<T>) -> Option<u64>;
    fn remove(&mut self, tx_id: u64);
    fn get(&self, tx_id: u64) -> Option<&RefCell<Tx<T>>>;
    fn iter(&self) -> Iter<u64, RefCell<Tx<T>>>;
}

impl TransactionsOps<TxSendData> for Transactions {
    fn insert(&mut self, tx: Tx<TxSendData>) -> Option<u64> {
        let tx_id = self.transaction_count;
        self.transaction_count += 1;
        self.send_data
            .insert(tx_id, RefCell::new(tx))
            .and_then(|_| Some(tx_id))
    }

    fn remove(&mut self, tx_id: u64) {
        self.send_data.remove(&tx_id);
    }

    fn get(&self, tx_id: u64) -> Option<&RefCell<Tx<TxSendData>>> {
        self.send_data.get(&tx_id)
    }

    fn iter(&self) -> Iter<u64, RefCell<Tx<TxSendData>>> {
        self.send_data.iter()
    }
}

impl TransactionsOps<TxSendChunk> for Transactions {
    fn insert(&mut self, tx: Tx<TxSendChunk>) -> Option<u64> {
        let tx_id = self.transaction_count;
        self.transaction_count += 1;
        self.send_chunk
            .insert(tx_id, RefCell::new(tx))
            .and_then(|_| Some(tx_id))
    }

    fn remove(&mut self, tx_id: u64) {
        self.send_chunk.remove(&tx_id);
    }

    fn get(&self, tx_id: u64) -> Option<&RefCell<Tx<TxSendChunk>>> {
        self.send_chunk.get(&tx_id)
    }

    fn iter(&self) -> Iter<u64, RefCell<Tx<TxSendChunk>>> {
        self.send_chunk.iter()
    }
}

impl TransactionsOps<TxSendConnectionMessage> for Transactions {
    fn insert(&mut self, tx: Tx<TxSendConnectionMessage>) -> Option<u64> {
        let tx_id = self.transaction_count;
        self.transaction_count += 1;
        self.send_connection_message
            .insert(tx_id, RefCell::new(tx))
            .and_then(|_| Some(tx_id))
    }

    fn remove(&mut self, tx_id: u64) {
        self.send_connection_message.remove(&tx_id);
    }

    fn get(&self, tx_id: u64) -> Option<&RefCell<Tx<TxSendConnectionMessage>>> {
        self.send_connection_message.get(&tx_id)
    }

    fn iter(&self) -> Iter<u64, RefCell<Tx<TxSendConnectionMessage>>> {
        self.send_connection_message.iter()
    }
}

impl TransactionsOps<TxSendMetadataMessage> for Transactions {
    fn insert(&mut self, tx: Tx<TxSendMetadataMessage>) -> Option<u64> {
        let tx_id = self.transaction_count;
        self.transaction_count += 1;
        self.send_metadata_message
            .insert(tx_id, RefCell::new(tx))
            .and_then(|_| Some(tx_id))
    }

    fn remove(&mut self, tx_id: u64) {
        self.send_metadata_message.remove(&tx_id);
    }

    fn get(&self, tx_id: u64) -> Option<&RefCell<Tx<TxSendMetadataMessage>>> {
        self.send_metadata_message.get(&tx_id)
    }

    fn iter(&self) -> Iter<u64, RefCell<Tx<TxSendMetadataMessage>>> {
        self.send_metadata_message.iter()
    }
}

impl TransactionsOps<TxSendAckMessage> for Transactions {
    fn insert(&mut self, tx: Tx<TxSendAckMessage>) -> Option<u64> {
        let tx_id = self.transaction_count;
        self.transaction_count += 1;
        self.send_ack_message
            .insert(tx_id, RefCell::new(tx))
            .and_then(|_| Some(tx_id))
    }

    fn remove(&mut self, tx_id: u64) {
        self.send_ack_message.remove(&tx_id);
    }

    fn get(&self, tx_id: u64) -> Option<&RefCell<Tx<TxSendAckMessage>>> {
        self.send_ack_message.get(&tx_id)
    }

    fn iter(&self) -> Iter<u64, RefCell<Tx<TxSendAckMessage>>> {
        self.send_ack_message.iter()
    }
}

impl TransactionsOps<TxRecvData> for Transactions {
    fn insert(&mut self, tx: Tx<TxRecvData>) -> Option<u64> {
        let tx_id = self.transaction_count;
        self.transaction_count += 1;
        self.recv_data
            .insert(tx_id, RefCell::new(tx))
            .and_then(|_| Some(tx_id))
    }

    fn remove(&mut self, tx_id: u64) {
        self.recv_data.remove(&tx_id);
    }

    fn get(&self, tx_id: u64) -> Option<&RefCell<Tx<TxRecvData>>> {
        self.recv_data.get(&tx_id)
    }

    fn iter(&self) -> Iter<u64, RefCell<Tx<TxRecvData>>> {
        self.recv_data.iter()
    }
}

impl TransactionsOps<TxRecvChunk> for Transactions {
    fn insert(&mut self, tx: Tx<TxRecvChunk>) -> Option<u64> {
        let tx_id = self.transaction_count;
        self.transaction_count += 1;
        self.recv_chunk
            .insert(tx_id, RefCell::new(tx))
            .and_then(|_| Some(tx_id))
    }

    fn remove(&mut self, tx_id: u64) {
        self.recv_chunk.remove(&tx_id);
    }

    fn get(&self, tx_id: u64) -> Option<&RefCell<Tx<TxRecvChunk>>> {
        self.recv_chunk.get(&tx_id)
    }

    fn iter(&self) -> Iter<u64, RefCell<Tx<TxRecvChunk>>> {
        self.recv_chunk.iter()
    }
}

impl TransactionsOps<TxRecvConnectionMessage> for Transactions {
    fn insert(&mut self, tx: Tx<TxRecvConnectionMessage>) -> Option<u64> {
        let tx_id = self.transaction_count;
        self.transaction_count += 1;
        self.recv_connection_message
            .insert(tx_id, RefCell::new(tx))
            .and_then(|_| Some(tx_id))
    }

    fn remove(&mut self, tx_id: u64) {
        self.recv_connection_message.remove(&tx_id);
    }

    fn get(&self, tx_id: u64) -> Option<&RefCell<Tx<TxRecvConnectionMessage>>> {
        self.recv_connection_message.get(&tx_id)
    }

    fn iter(&self) -> Iter<u64, RefCell<Tx<TxRecvConnectionMessage>>> {
        self.recv_connection_message.iter()
    }
}

impl TransactionsOps<TxRecvMetadataMessage> for Transactions {
    fn insert(&mut self, tx: Tx<TxRecvMetadataMessage>) -> Option<u64> {
        let tx_id = self.transaction_count;
        self.transaction_count += 1;
        self.recv_metadata_message
            .insert(tx_id, RefCell::new(tx))
            .and_then(|_| Some(tx_id))
    }

    fn remove(&mut self, tx_id: u64) {
        self.recv_metadata_message.remove(&tx_id);
    }

    fn get(&self, tx_id: u64) -> Option<&RefCell<Tx<TxRecvMetadataMessage>>> {
        self.recv_metadata_message.get(&tx_id)
    }

    fn iter(&self) -> Iter<u64, RefCell<Tx<TxRecvMetadataMessage>>> {
        self.recv_metadata_message.iter()
    }
}

impl TransactionsOps<TxRecvAckMessage> for Transactions {
    fn insert(&mut self, tx: Tx<TxRecvAckMessage>) -> Option<u64> {
        let tx_id = self.transaction_count;
        self.transaction_count += 1;
        self.recv_ack_message
            .insert(tx_id, RefCell::new(tx))
            .and_then(|_| Some(tx_id))
    }

    fn remove(&mut self, tx_id: u64) {
        self.recv_ack_message.remove(&tx_id);
    }

    fn get(&self, tx_id: u64) -> Option<&RefCell<Tx<TxRecvAckMessage>>> {
        self.recv_ack_message.get(&tx_id)
    }

    fn iter(&self) -> Iter<u64, RefCell<Tx<TxRecvAckMessage>>> {
        self.recv_ack_message.iter()
    }
}

impl TransactionsOps<TxHandshake> for Transactions {
    fn insert(&mut self, tx: Tx<TxHandshake>) -> Option<u64> {
        let tx_id = self.transaction_count;
        self.transaction_count += 1;
        self.handshake
            .insert(tx_id, RefCell::new(tx))
            .and_then(|_| Some(tx_id))
    }

    fn remove(&mut self, tx_id: u64) {
        self.handshake.remove(&tx_id);
    }

    fn get(&self, tx_id: u64) -> Option<&RefCell<Tx<TxHandshake>>> {
        self.handshake.get(&tx_id)
    }

    fn iter(&self) -> Iter<u64, RefCell<Tx<TxHandshake>>> {
        self.handshake.iter()
    }
}

pub struct GlobalState {
    pub transactions: RefCell<Transactions>, // each transaction has its own local-state
    pub shared: SharedState,                 // shared state (mutally-exclusive access permissions)
    pub immutable: ImmutableState, // immutable state (read-only, can be accessed directly)
}

impl GlobalState {
    pub fn new(immutable: ImmutableState) -> Self {
        Self {
            transactions: RefCell::new(Transactions::default()),
            shared: SharedState::new(),
            immutable,
        }
    }

    pub fn access(&self, tx_id: u64) -> FieldAccess {
        self.transactions.borrow().transaction_access(tx_id)
    }
}
