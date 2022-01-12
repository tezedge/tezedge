use crate::{
    action::PureAction,
    state::{GlobalState, SharedStateAccess, SharedStateField},
};
use enum_dispatch::enum_dispatch;

use super::{
    chunk::{TxRecvChunk, TxSendChunk},
    data_transfer::{TxRecvData, TxSendData},
    handshake::{
        ack::{TxRecvAckMessage, TxSendAckMessage},
        connection::{TxRecvConnectionMessage, TxSendConnectionMessage},
        handshake::TxHandshake,
        metadata::{TxRecvMetadataMessage, TxSendMetadataMessage},
    },
};

#[enum_dispatch(TransactionType)]
pub trait Transaction {
    fn init_reducer_fn(&self) -> fn(u64, &mut GlobalState);
    fn commit_reducer_fn(&self) -> fn(u64, &mut GlobalState);
}

#[enum_dispatch]
#[derive(Clone)]
pub enum TransactionType {
    TxSendData,
    TxSendChunk,
    TxSendConnectionMessage,
    TxSendMetadataMessage,
    TxSendAckMessage,
    TxRecvData,
    TxRecvChunk,
    TxRecvConnectionMessage,
    TxRecvMetadataMessage,
    TxRecvAckMessage,
    TxHandshake,
}

#[derive(PartialEq, Clone)]
pub enum TransactionStatus {
    Pending,
    Retry,
    Cancelled,
    Completed,
}

pub trait TransactionStage {
    fn is_stage_enabled(&self, stage: &Self) -> bool;

    fn set_stage(&mut self, new_stage: Self)
    where
        Self: Sized,
    {
        assert!(self.is_stage_enabled(&new_stage));
        *self = new_stage;
    }
}

pub struct Tx {
    tx_type: TransactionType,
    state_access: SharedStateAccess,
    transaction_status: TransactionStatus,
}

impl Tx {
    pub fn new(
        tx_type: TransactionType,
        state_access: SharedStateAccess,
        transaction_status: TransactionStatus,
    ) -> Self {
        Tx {
            tx_type,
            state_access,
            transaction_status,
        }
    }

    pub fn tx_type(&self) -> &TransactionType {
        &self.tx_type
    }

    pub fn tx_type_mut(&mut self) -> &mut TransactionType {
        &mut self.tx_type
    }

    pub fn access(&self) -> &SharedStateAccess {
        &self.state_access
    }

    pub fn is_status(&self, status: TransactionStatus) -> bool {
        self.transaction_status == status
    }

    pub fn is_pending(&self) -> bool {
        self.is_status(TransactionStatus::Pending)
    }

    pub fn is_completed(&self) -> bool {
        self.is_status(TransactionStatus::Completed)
    }

    pub fn is_cancelled(&self) -> bool {
        self.is_status(TransactionStatus::Cancelled)
    }
}

pub struct CreateTransactionAction {
    tx_access: SharedStateAccess,
    tx_type: TransactionType,
}

impl CreateTransactionAction {
    pub fn new(
        access_read: &[SharedStateField],
        access_write: &[SharedStateField],
        tx_type: TransactionType,
    ) -> Self {
        Self {
            tx_access: SharedStateAccess::new(access_read, access_write),
            tx_type: tx_type,
        }
    }
}

impl PureAction for CreateTransactionAction {
    fn reducer(&self, state: &mut GlobalState) {
        let status = match state.is_tx_exclusive(&self.tx_access) {
            true => TransactionStatus::Retry,
            false => TransactionStatus::Pending,
        };

        let tx_id = state.add_transaction(Tx::new(
            self.tx_type.clone(),
            self.tx_access.clone(),
            status.clone(),
        ));

        if status == TransactionStatus::Pending {
            StartTransactionAction::new(tx_id).dispatch_pure(state);
        }
    }
}

pub struct StartTransactionAction {
    tx_id: u64,
}

impl StartTransactionAction {
    pub fn new(tx_id: u64) -> Self {
        Self { tx_id }
    }
}

impl PureAction for StartTransactionAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        assert!(transaction.is_pending());

        let tx_state = transaction.tx_type_mut();
        let init_reducer = tx_state.init_reducer_fn();

        drop(transactions);
        init_reducer(self.tx_id, state);
    }
}

pub struct CompleteTransactionAction {
    tx_id: u64,
}

impl CompleteTransactionAction {
    pub fn new(tx_id: u64) -> Self {
        Self { tx_id }
    }
}

impl PureAction for CompleteTransactionAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let mut transaction = transactions.get_mut(&self.tx_id).unwrap();

        assert!(transaction.is_pending());
        transaction.transaction_status = TransactionStatus::Completed;

        let tx_state = transaction.tx_type_mut();
        let commit_reducer = tx_state.commit_reducer_fn();

        drop(transactions);
        // Shared-state write access is granted to `commit_reducer`
        commit_reducer(self.tx_id, state);

        let transaction = state.remove_transaction(self.tx_id);
        assert!(transaction.is_completed());
    }
}

pub enum CancelTransactionReason {
    ConnectionClosed,
}

pub struct CancelTransactionAction {
    tx_id: u64,
    reason: CancelTransactionReason,
}

impl CancelTransactionAction {
    pub fn new(tx_id: u64, reason: CancelTransactionReason) -> Self {
        Self { tx_id, reason }
    }
}

impl PureAction for CancelTransactionAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let mut transaction = transactions.get_mut(&self.tx_id).unwrap();

        assert!(!transaction.is_cancelled() && !transaction.is_completed());
        transaction.transaction_status = TransactionStatus::Cancelled;

        let transaction = state.remove_transaction(self.tx_id);
        assert!(transaction.is_cancelled());
    }
}
