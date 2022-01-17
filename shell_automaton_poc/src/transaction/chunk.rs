use std::mem::size_of;

use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryChunkError};

use crate::{
    action::PureAction,
    state::GlobalState,
    transaction::transaction::{TransactionState, TxOps},
};

use super::{
    data_transfer::{TxRecvData, TxSendData},
    handshake::{
        ack::{RecvAckMessageAction, SendAckMessageCompleteAction},
        connection::{RecvConnectionMessageAction, SendConnectionMessageCompleteAction},
        metadata::{RecvMetadataMessageAction, SendMetadataMessageCompleteAction},
    },
    transaction::{
        CompleteTransactionAction, CreateTransactionAction, ParentInfo, Transaction,
        TransactionStage, TransactionType,
    },
};

#[derive(Clone)]
pub enum TxRecvChunk {
    ReceivingHeader(mio::Token),
    ReceivingBody,
    Completed(Result<Vec<u8>, ()>),
}

impl TransactionStage for TxRecvChunk {
    fn is_enabled(&self, next_stage: &Self) -> bool {
        match self {
            Self::ReceivingHeader(_) => match next_stage {
                Self::Completed(Err(_)) => true,
                Self::ReceivingBody => true,
                _ => false,
            },
            Self::ReceivingBody => next_stage.is_completed(),
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

impl Transaction for TxRecvChunk {
    fn init(&mut self, tx_id: u64, state: &GlobalState) {
        match *self {
            TxRecvChunk::ReceivingHeader(token) => {
                // create child transaction to receive the chunk length
                CreateTransactionAction::new(
                    0,
                    0,
                    TxRecvData::new(token, size_of::<u16>()),
                    Some(ParentInfo {
                        tx_id,
                        tx_type: TransactionType::TxRecvChunk,
                    }),
                )
                .dispatch_pure(state);
            }
            _ => panic!("Invalid intitial state"),
        }
    }

    fn commit(&self, _tx_id: u64, parent: Option<ParentInfo>, state: &GlobalState) {
        assert!(self.is_completed() && parent.is_some());

        if let TxRecvChunk::Completed(result) = self {
            if let Some(ParentInfo { tx_id, tx_type }) = parent {
                match tx_type {
                    TransactionType::TxRecvConnectionMessage => {
                        RecvConnectionMessageAction::new(tx_id, result.clone())
                            .dispatch_pure(state);
                    }
                    TransactionType::TxRecvMetadataMessage => {
                        RecvMetadataMessageAction::new(tx_id, result.clone()).dispatch_pure(state);
                    }
                    TransactionType::TxRecvAckMessage => {
                        RecvAckMessageAction::new(tx_id, result.clone()).dispatch_pure(state);
                    }
                    _ => panic!("Unhandled parent transaction type"),
                }
            }
        }
    }

    fn cancel(&self, _tx_id: u64, _parent: Option<ParentInfo>, _state: &GlobalState) {}
}

impl TxRecvChunk {
    pub fn new(token: mio::Token) -> Self {
        Self::ReceivingHeader(token)
    }
}

pub struct RecvChunkAction {
    tx_id: u64,
    result: Result<Vec<u8>, ()>,
}

impl RecvChunkAction {
    pub fn new(tx_id: u64, result: Result<Vec<u8>, ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for RecvChunkAction {
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = TxRecvChunk::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            match tx_state.clone() {
                TxRecvChunk::ReceivingHeader(token) => {
                    if let Ok(data) = &self.result {
                        // TODO: proper handling
                        let chunk_len =
                            u16::from_be_bytes(data.as_slice().try_into().unwrap()) as usize;

                        if chunk_len != 0 {
                            tx_state.set(TxRecvChunk::ReceivingBody);
                            // create child transaction to receive the chunk body
                            //drop(transaction);
                            //drop(transactions);
                            CreateTransactionAction::new(
                                0,
                                0,
                                TxRecvData::new(token, chunk_len),
                                Some(ParentInfo {
                                    tx_id: self.tx_id,
                                    tx_type: TransactionType::TxRecvChunk,
                                }),
                            )
                            .dispatch_pure(state);
                            return;
                        }
                    }

                    tx_state.set(TxRecvChunk::Completed(Err(())));
                    //drop(transaction);
                    //drop(transactions);
                    CompleteTransactionAction::<TxRecvChunk>::new(self.tx_id).dispatch_pure(state);
                }
                TxRecvChunk::ReceivingBody => {
                    tx_state.set(TxRecvChunk::Completed(self.result.clone()));
                    //drop(transaction);
                    //drop(transactions);
                    CompleteTransactionAction::<TxRecvChunk>::new(self.tx_id).dispatch_pure(state);
                }
                _ => panic!("RecvChunkAction reducer: action dispatched at invalid stage"),
            }
        }
    }
}

#[derive(Clone)]
pub enum TxSendChunk {
    Transmitting(mio::Token, Vec<u8>),
    Completed(Result<(), ()>),
}

impl TransactionStage for TxSendChunk {
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

impl Transaction for TxSendChunk {
    fn init(&mut self, tx_id: u64, state: &GlobalState) {
        match self {
            TxSendChunk::Transmitting(token, bytes_to_send) => {
                CreateTransactionAction::new(
                    0,
                    0,
                    TxSendData::new(*token, bytes_to_send.clone()),
                    Some(ParentInfo {
                        tx_id,
                        tx_type: TransactionType::TxSendChunk,
                    }),
                )
                .dispatch_pure(state);
            }
            _ => panic!("Invalid intitial state"),
        }
    }

    fn commit(&self, _tx_id: u64, parent: Option<ParentInfo>, state: &GlobalState) {
        assert!(self.is_completed() && parent.is_some());

        if let TxSendChunk::Completed(result) = *self {
            if let Some(ParentInfo { tx_id, tx_type }) = parent {
                match tx_type {
                    TransactionType::TxSendConnectionMessage => {
                        SendConnectionMessageCompleteAction::new(tx_id, result)
                            .dispatch_pure(state);
                    }
                    TransactionType::TxSendMetadataMessage => {
                        SendMetadataMessageCompleteAction::new(tx_id, result).dispatch_pure(state);
                    }
                    TransactionType::TxSendAckMessage => {
                        SendAckMessageCompleteAction::new(tx_id, result).dispatch_pure(state);
                    }
                    _ => panic!("Unhandled parent transaction type"),
                }
            }
        }
    }

    fn cancel(&self, _tx_id: u64, _parent: Option<ParentInfo>, _state: &GlobalState) {}
}

impl TxSendChunk {
    pub fn try_new(token: mio::Token, bytes_to_send: Vec<u8>) -> Result<Self, BinaryChunkError> {
        let chunk = BinaryChunk::from_content(bytes_to_send.as_slice())?;
        Ok(Self::Transmitting(token, chunk.raw().to_vec()))
    }

    fn is_transmitting(&self) -> bool {
        match self {
            Self::Transmitting(..) => true,
            _ => false,
        }
    }
}

pub struct SendChunkCompleteAction {
    tx_id: u64,
    result: Result<(), ()>,
}

impl SendChunkCompleteAction {
    pub fn new(tx_id: u64, result: Result<(), ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for SendChunkCompleteAction {
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = TxSendChunk::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            assert!(tx_state.is_transmitting());
            tx_state.set(TxSendChunk::Completed(self.result));
            //drop(transaction);
            //drop(transactions);
            CompleteTransactionAction::<TxSendChunk>::new(self.tx_id).dispatch_pure(state);
        }
    }
}
