use crypto::{crypto_box::PrecomputedKey, nonce::Nonce};
use tezos_messages::p2p::{
    binary_message::{BinaryRead, BinaryWrite},
    encoding::ack::AckMessage,
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

use super::handshake::{HandshakeRecvAckMessageAction, HandshakeSendAckMessageCompleteAction};

#[derive(Clone)]
pub struct RecvAckMessageContext {
    pub token: mio::Token,
    pub nonce: Nonce,
    pub key: PrecomputedKey,
}

#[derive(Clone)]
pub enum TxRecvAckMessage {
    Receiving(RecvAckMessageContext),
    Completed(Nonce, Result<Vec<u8>, ()>),
}

impl TransactionStage for TxRecvAckMessage {
    fn is_enabled(&self, next_stage: &Self) -> bool {
        match self {
            Self::Receiving(_) => next_stage.is_completed(),
            _ => false,
        }
    }

    fn is_completed(&self) -> bool {
        match self {
            Self::Completed(..) => true,
            _ => false,
        }
    }
}

impl Transaction for TxRecvAckMessage {
    fn init(&mut self, tx_id: u64, state: &GlobalState) {
        match self {
            TxRecvAckMessage::Receiving(ctx) => {
                CreateTransactionAction::new(
                    0,
                    0,
                    TxRecvChunk::new(ctx.token),
                    Some(ParentInfo {
                        tx_id,
                        tx_type: TransactionType::TxRecvAckMessage,
                    }),
                )
                .dispatch_pure(state);
            }
            _ => panic!("Invalid intitial state"),
        }
    }

    fn commit(&self, _tx_id: u64, parent: Option<ParentInfo>, state: &GlobalState) {
        assert!(self.is_completed() && parent.is_some());

        if let TxRecvAckMessage::Completed(nonce, result) = self {
            let result = result
                .clone()
                .and_then(|bytes| AckMessage::from_bytes(bytes).or_else(|_| Err(())));

            if let Some(ParentInfo { tx_id, tx_type }) = parent {
                match tx_type {
                    TransactionType::TxHandshake => {
                        HandshakeRecvAckMessageAction::new(tx_id, nonce.clone(), result)
                            .dispatch_pure(state);
                    }
                    _ => panic!("Unhandled parent transaction type"),
                }
            }
        }
    }

    fn cancel(&self, _tx_id: u64, _parent: Option<ParentInfo>, _state: &GlobalState) {}
}

impl TxRecvAckMessage {
    pub fn new(token: mio::Token, nonce: Nonce, key: PrecomputedKey) -> Self {
        Self::Receiving(RecvAckMessageContext { token, nonce, key })
    }

    fn is_receiving(&self) -> bool {
        match self {
            Self::Receiving(_) => true,
            _ => false,
        }
    }
}

pub struct RecvAckMessageAction {
    tx_id: u64,
    result: Result<Vec<u8>, ()>,
}

impl RecvAckMessageAction {
    pub fn new(tx_id: u64, result: Result<Vec<u8>, ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for RecvAckMessageAction {
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = TxRecvAckMessage::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            assert!(tx_state.is_receiving());

            if let TxRecvAckMessage::Receiving(ctx) = tx_state.clone() {
                let result = self
                    .result
                    .clone()
                    .and_then(|bytes| ctx.key.decrypt(&bytes, &ctx.nonce).or_else(|_| Err(())));

                tx_state.set(TxRecvAckMessage::Completed(ctx.nonce.increment(), result));
                //drop(transaction);
                //drop(transactions);
                CompleteTransactionAction::<TxRecvAckMessage>::new(self.tx_id).dispatch_pure(state);
            }
        }
    }
}

#[derive(Clone)]
pub struct SendAckMessageContext {
    pub token: mio::Token,
    pub nonce: Nonce,
    pub bytes_to_send: Vec<u8>,
}

#[derive(Clone)]
pub enum TxSendAckMessage {
    Transmitting(SendAckMessageContext),
    Completed(Nonce, Result<(), ()>),
}

impl TransactionStage for TxSendAckMessage {
    fn is_enabled(&self, next_stage: &Self) -> bool {
        match self {
            Self::Transmitting(_) => next_stage.is_completed(),
            _ => false,
        }
    }

    fn is_completed(&self) -> bool {
        match self {
            Self::Completed(..) => true,
            _ => false,
        }
    }
}

impl Transaction for TxSendAckMessage {
    fn init(&mut self, tx_id: u64, state: &GlobalState) {
        match self {
            TxSendAckMessage::Transmitting(ctx) => {
                CreateTransactionAction::new(
                    0,
                    0,
                    TxSendChunk::try_new(ctx.token, ctx.bytes_to_send.clone()).unwrap(),
                    Some(ParentInfo {
                        tx_id,
                        tx_type: TransactionType::TxSendAckMessage,
                    }),
                )
                .dispatch_pure(state);
            }
            _ => panic!("Invalid intitial state"),
        }
    }

    fn commit(&self, _tx_id: u64, parent: Option<ParentInfo>, state: &GlobalState) {
        assert!(self.is_completed() && parent.is_some());

        if let TxSendAckMessage::Completed(nonce, result) = self {
            if let Some(ParentInfo { tx_id, tx_type }) = parent {
                match tx_type {
                    TransactionType::TxHandshake => {
                        HandshakeSendAckMessageCompleteAction::new(
                            tx_id,
                            nonce.clone(),
                            result.clone(),
                        )
                        .dispatch_pure(state);
                    }
                    _ => panic!("Unhandled parent transaction type"),
                }
            }
        }
    }

    fn cancel(&self, _tx_id: u64, _parent: Option<ParentInfo>, _state: &GlobalState) {}
}

impl TxSendAckMessage {
    pub fn try_new(
        token: mio::Token,
        nonce: Nonce,
        key: PrecomputedKey,
        msg: AckMessage,
    ) -> Result<Self, ()> {
        let bytes = msg.as_bytes().or_else(|_| Err(()))?;
        let bytes_to_send = key.encrypt(&bytes, &nonce).or_else(|_| Err(()))?;

        Ok(Self::Transmitting(SendAckMessageContext {
            token,
            nonce,
            bytes_to_send,
        }))
    }

    fn is_transmitting(&self) -> bool {
        match self {
            Self::Transmitting(_) => true,
            _ => false,
        }
    }
}

pub struct SendAckMessageCompleteAction {
    tx_id: u64,
    result: Result<(), ()>,
}

impl SendAckMessageCompleteAction {
    pub fn new(tx_id: u64, result: Result<(), ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for SendAckMessageCompleteAction {
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = TxSendAckMessage::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            assert!(tx_state.is_transmitting());

            if let TxSendAckMessage::Transmitting(ctx) = tx_state.clone() {
                tx_state.set(TxSendAckMessage::Completed(
                    ctx.nonce.increment(),
                    self.result,
                ));
                //drop(transaction);
                //drop(transactions);
                CompleteTransactionAction::<TxSendAckMessage>::new(self.tx_id).dispatch_pure(state);
            }
        }
    }
}
