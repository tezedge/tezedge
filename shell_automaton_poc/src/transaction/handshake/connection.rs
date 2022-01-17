use tezos_encoding::binary_writer::BinaryWriterError;
use tezos_messages::p2p::{
    binary_message::{BinaryRead, BinaryWrite},
    encoding::connection::ConnectionMessage,
};

use crate::{
    action::PureAction,
    state::GlobalState,
    transaction::{
        chunk::{TxRecvChunk, TxSendChunk},
        transaction::{
            CompleteTransactionAction, CreateTransactionAction, ParentInfo, Transaction,
            TransactionStage, TransactionState, TransactionType, TxOps,
        },
    },
};

use super::handshake::{
    HandshakeRecvConnectionMessageAction, HandshakeSendConnectionMessageCompleteAction,
};

#[derive(Clone)]
pub enum TxRecvConnectionMessage {
    Receiving(mio::Token),
    Completed(Result<Vec<u8>, ()>),
}

impl TransactionStage for TxRecvConnectionMessage {
    fn is_enabled(&self, next_stage: &Self) -> bool {
        match self {
            Self::Receiving(_) => next_stage.is_completed(),
            _ => false,
        }
    }

    fn is_completed(&self) -> bool {
        match self {
            Self::Completed(_) => true,
            _ => false,
        }
    }
}

impl Transaction for TxRecvConnectionMessage {
    fn init(&mut self, tx_id: u64, state: &GlobalState) {
        match self {
            TxRecvConnectionMessage::Receiving(token) => {
                CreateTransactionAction::new(
                    0,
                    0,
                    TxRecvChunk::new(token.clone()),
                    Some(ParentInfo {
                        tx_id,
                        tx_type: TransactionType::TxRecvConnectionMessage,
                    }),
                )
                .dispatch_pure(state);
            }
            _ => panic!("Invalid intitial state"),
        }
    }

    fn commit(&self, _tx_id: u64, parent: Option<ParentInfo>, state: &GlobalState) {
        assert!(self.is_completed() && parent.is_some());

        if let TxRecvConnectionMessage::Completed(result) = self {
            let result = result
                .clone()
                .and_then(|bytes| ConnectionMessage::from_bytes(bytes).or_else(|_| Err(())));

            if let Some(ParentInfo { tx_id, tx_type }) = parent {
                match tx_type {
                    TransactionType::TxHandshake => {
                        HandshakeRecvConnectionMessageAction::new(tx_id, result)
                            .dispatch_pure(state);
                    }
                    _ => panic!("Unhandled parent transaction type"),
                }
            }
        }
    }

    fn cancel(&self, _tx_id: u64, _parent: Option<ParentInfo>, _state: &GlobalState) {}
}

impl TxRecvConnectionMessage {
    pub fn new(token: mio::Token) -> Self {
        Self::Receiving(token)
    }

    fn is_receiving(&self) -> bool {
        match self {
            Self::Receiving(_) => true,
            _ => false,
        }
    }
}

pub struct RecvConnectionMessageAction {
    tx_id: u64,
    result: Result<Vec<u8>, ()>,
}

impl RecvConnectionMessageAction {
    pub fn new(tx_id: u64, result: Result<Vec<u8>, ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for RecvConnectionMessageAction {
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = TxRecvConnectionMessage::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            assert!(tx_state.is_receiving());
            tx_state.set(TxRecvConnectionMessage::Completed(self.result.clone()));
            //drop(transaction);
            //drop(transactions);
            CompleteTransactionAction::<TxRecvConnectionMessage>::new(self.tx_id)
                .dispatch_pure(state);
        }
    }
}

#[derive(Clone)]
pub enum TxSendConnectionMessage {
    Transmitting(mio::Token, Vec<u8>),
    Completed(Result<(), ()>),
}

impl TransactionStage for TxSendConnectionMessage {
    fn is_enabled(&self, next_stage: &Self) -> bool {
        match self {
            Self::Transmitting(..) => next_stage.is_completed(),
            _ => false,
        }
    }

    fn is_completed(&self) -> bool {
        match self {
            Self::Completed(_) => true,
            _ => false,
        }
    }
}

impl Transaction for TxSendConnectionMessage {
    fn init(&mut self, tx_id: u64, state: &GlobalState) {
        match self {
            TxSendConnectionMessage::Transmitting(token, bytes_to_send) => {
                CreateTransactionAction::new(
                    0,
                    0,
                    TxSendChunk::try_new(token.clone(), bytes_to_send.clone()).unwrap(),
                    Some(ParentInfo {
                        tx_id,
                        tx_type: TransactionType::TxSendConnectionMessage,
                    }),
                )
                .dispatch_pure(state);
            }
            _ => panic!("Invalid intitial state"),
        }
    }

    fn commit(&self, _tx_id: u64, parent: Option<ParentInfo>, state: &GlobalState) {
        assert!(self.is_completed() && parent.is_some());

        if let TxSendConnectionMessage::Completed(result) = self {
            if let Some(ParentInfo { tx_id, tx_type }) = parent {
                match tx_type {
                    TransactionType::TxHandshake => {
                        HandshakeSendConnectionMessageCompleteAction::new(tx_id, result.clone())
                            .dispatch_pure(state);
                    }
                    _ => panic!("Unhandled parent transaction type"),
                }
            }
        }
    }

    fn cancel(&self, _tx_id: u64, _parent: Option<ParentInfo>, _state: &GlobalState) {}
}

impl TxSendConnectionMessage {
    pub fn try_new(token: mio::Token, msg: ConnectionMessage) -> Result<Self, BinaryWriterError> {
        let bytes_to_send = msg.as_bytes()?;
        Ok(Self::Transmitting(token, bytes_to_send))
    }

    fn is_transmitting(&self) -> bool {
        match self {
            Self::Transmitting(..) => true,
            _ => false,
        }
    }
}

pub struct SendConnectionMessageCompleteAction {
    tx_id: u64,
    result: Result<(), ()>,
}

impl SendConnectionMessageCompleteAction {
    pub fn new(tx_id: u64, result: Result<(), ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for SendConnectionMessageCompleteAction {
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = TxSendConnectionMessage::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            assert!(tx_state.is_transmitting());
            tx_state.set(TxSendConnectionMessage::Completed(self.result));
            //drop(transaction);
            //drop(transactions);
            CompleteTransactionAction::<TxSendConnectionMessage>::new(self.tx_id)
                .dispatch_pure(state);
        }
    }
}
