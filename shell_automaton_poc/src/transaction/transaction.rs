use std::{
    cell::{RefCell, RefMut},
    collections::btree_map::Iter,
    marker::PhantomData,
};

use crate::{
    action::PureAction,
    state::{FieldAccess, FieldAccessMask, GlobalState, Transactions, TransactionsOps},
};

#[derive(Clone)]
pub struct ParentInfo {
    pub tx_id: u64,
    pub tx_type: TransactionType,
}

pub trait Transaction
where
    Self: Sized,
    Transactions: TransactionsOps<Self>,
{
    fn init(&mut self, tx_id: u64, state: &GlobalState);
    fn commit(&self, tx_id: u64, parent: Option<ParentInfo>, state: &GlobalState);
    fn cancel(&self, tx_id: u64, parent: Option<ParentInfo>, state: &GlobalState);

    fn transactions(transactions: &Transactions) -> Iter<u64, RefCell<Tx<Self>>> {
        transactions.iter()
    }

    fn get(transactions: &Transactions, tx_id: u64) -> &RefCell<Tx<Self>> {
        transactions
            .get(tx_id)
            .unwrap_or_else(|| panic!("Transaction not found"))
    }

    fn remove(mut transactions: RefMut<Transactions>, tx_id: u64) {
        transactions.remove(tx_id);
    }
}

#[derive(Clone, Copy)]
pub enum TransactionType {
    TxSendChunk,
    TxSendConnectionMessage,
    TxSendMetadataMessage,
    TxSendAckMessage,
    TxRecvChunk,
    TxRecvConnectionMessage,
    TxRecvMetadataMessage,
    TxRecvAckMessage,
    TxHandshake,
}

#[derive(PartialEq, Clone)]
pub enum TransactionState<T: Transaction>
where
    Transactions: TransactionsOps<T>,
{
    Pending(T),
    Retry(T),
}

pub trait TransactionStage {
    fn is_enabled(&self, stage: &Self) -> bool;
    fn is_completed(&self) -> bool;

    fn set(&mut self, new_stage: Self)
    where
        Self: Sized,
    {
        assert!(self.is_enabled(&new_stage));
        *self = new_stage;
    }
}

pub trait TxOps {
    fn access(&self) -> &FieldAccess;
    fn parent(&self) -> Option<ParentInfo>;
    fn is_pending(&self) -> bool;
    fn is_retry(&self) -> bool;
}

pub struct Tx<T: Transaction>
where
    Transactions: TransactionsOps<T>,
{
    pub state: TransactionState<T>,
    parent: Option<ParentInfo>,
    access: FieldAccess,
}

impl<T: Transaction> Tx<T>
where
    Transactions: TransactionsOps<T>,
{
    pub fn new(
        state: TransactionState<T>,
        parent: Option<ParentInfo>,
        access: FieldAccess,
    ) -> Self {
        Tx {
            state,
            parent,
            access,
        }
    }
}

impl<T: Transaction> TxOps for Tx<T>
where
    Transactions: TransactionsOps<T>,
{
    fn access(&self) -> &FieldAccess {
        &self.access
    }

    fn parent(&self) -> Option<ParentInfo> {
        self.parent.clone()
    }

    fn is_pending(&self) -> bool {
        match self.state {
            TransactionState::Pending(_) => true,
            _ => false,
        }
    }

    fn is_retry(&self) -> bool {
        match self.state {
            TransactionState::Retry(_) => true,
            _ => false,
        }
    }
}

pub struct CreateTransactionAction<T: Transaction>
where
    Transactions: TransactionsOps<T>,
    T: Clone,
{
    transaction_access: FieldAccess,
    transaction_type: T,
    parent: Option<ParentInfo>,
}

impl<T: Transaction> CreateTransactionAction<T>
where
    Transactions: TransactionsOps<T>,
    T: Clone,
{
    pub fn new(
        read_access: FieldAccessMask,
        write_access: FieldAccessMask,
        transaction_type: T,
        parent: Option<ParentInfo>,
    ) -> Self {
        Self {
            transaction_access: FieldAccess::new(read_access, write_access),
            transaction_type,
            parent,
        }
    }
}

impl<T: Transaction> PureAction for CreateTransactionAction<T>
where
    Transactions: TransactionsOps<T>,
    T: Clone,
{
    fn reducer(&self, state: &GlobalState) {
        let tx_state = match state
            .transactions
            .borrow()
            .is_access_exclusive(self.parent.clone(), &self.transaction_access)
        {
            true => TransactionState::Retry(self.transaction_type.clone()),
            false => TransactionState::Pending(self.transaction_type.clone()),
        };

        let transaction = Tx::new(tx_state, self.parent.clone(), self.transaction_access);
        let is_pending = transaction.is_pending();
        let tx_id = match state.transactions.borrow_mut().insert(transaction) {
            Some(tx_id) => tx_id,
            None => panic!("Attempt to insert an existing transaction"),
        };

        if is_pending {
            StartTransactionAction::new(tx_id).dispatch_pure(state);
        }
    }
}

pub struct StartTransactionAction<T: Transaction>
where
    Transactions: TransactionsOps<T>,
{
    tx_id: u64,
    transaction_type: PhantomData<T>,
}

impl<T: Transaction> StartTransactionAction<T>
where
    Transactions: TransactionsOps<T>,
{
    pub fn new(tx_id: u64) -> Self {
        Self {
            tx_id,
            transaction_type: PhantomData,
        }
    }
}

impl<T: Transaction> PureAction for StartTransactionAction<T>
where
    Transactions: TransactionsOps<T>,
{
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = T::get(&transactions, self.tx_id).borrow_mut();

        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            //drop(transaction);
            //drop(transactions);
            tx_state.init(self.tx_id, state)
        }
    }
}

pub struct CompleteTransactionAction<T: Transaction>
where
    Transactions: TransactionsOps<T>,
{
    tx_id: u64,
    transaction_type: PhantomData<T>,
}

impl<T: Transaction> CompleteTransactionAction<T>
where
    Transactions: TransactionsOps<T>,
{
    pub fn new(tx_id: u64) -> Self {
        Self {
            tx_id,
            transaction_type: PhantomData,
        }
    }
}

impl<T: Transaction> PureAction for CompleteTransactionAction<T>
where
    Transactions: TransactionsOps<T>,
{
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let transaction = T::get(&transactions, self.tx_id).borrow();

        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &transaction.state {
            let parent = transaction.parent();
            //drop(transaction);
            //drop(transactions);
            tx_state.commit(self.tx_id, parent, state);
        }

        T::remove(state.transactions.borrow_mut(), self.tx_id);
    }
}

#[derive(Debug)]
pub enum CancelTransactionReason {
    ConnectionClosed,
}

#[derive(Debug)]
pub struct CancelTransactionAction<T: Transaction>
where
    Transactions: TransactionsOps<T>,
{
    tx_id: u64,
    _reason: CancelTransactionReason,
    transaction_type: PhantomData<T>,
}

impl<T: Transaction> CancelTransactionAction<T>
where
    Transactions: TransactionsOps<T>,
{
    pub fn new(tx_id: u64, _reason: CancelTransactionReason) -> Self {
        Self {
            tx_id,
            _reason,
            transaction_type: PhantomData,
        }
    }
}

impl<T: Transaction> PureAction for CancelTransactionAction<T>
where
    Transactions: TransactionsOps<T>,
{
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let transaction = T::get(&transactions, self.tx_id).borrow();

        if let TransactionState::Pending(tx_state) = &transaction.state {
            //drop(transaction);
            //drop(transactions);
            tx_state.cancel(self.tx_id, transaction.parent(), state)
        }

        T::remove(state.transactions.borrow_mut(), self.tx_id);
    }
}
