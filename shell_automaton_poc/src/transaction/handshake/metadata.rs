use crypto::{crypto_box::PrecomputedKey, nonce::Nonce};
use tezos_messages::p2p::{
    binary_message::{BinaryRead, BinaryWrite},
    encoding::metadata::MetadataMessage,
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
    HandshakeRecvMetadataMessageAction, HandshakeSendMetadataMessageCompleteAction,
};

#[derive(Clone)]
pub struct RecvMetadataMessageContext {
    pub token: mio::Token,
    pub nonce: Nonce,
    pub key: PrecomputedKey,
}

#[derive(Clone)]
pub enum TxRecvMetadataMessage {
    Receiving(RecvMetadataMessageContext),
    Completed(Nonce, Result<Vec<u8>, ()>),
}

impl TransactionStage for TxRecvMetadataMessage {
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

impl Transaction for TxRecvMetadataMessage {
    fn init(&mut self, tx_id: u64, state: &GlobalState) {
        match self {
            TxRecvMetadataMessage::Receiving(ctx) => {
                CreateTransactionAction::new(
                    0,
                    0,
                    TxRecvChunk::new(ctx.token),
                    Some(ParentInfo {
                        tx_id,
                        tx_type: TransactionType::TxRecvMetadataMessage,
                    }),
                )
                .dispatch_pure(state);
            }
            _ => panic!("Invalid intitial state"),
        }
    }

    fn commit(&self, _tx_id: u64, parent: Option<ParentInfo>, state: &GlobalState) {
        assert!(self.is_completed() && parent.is_some());

        if let TxRecvMetadataMessage::Completed(nonce, result) = self {
            let result = result
                .clone()
                .and_then(|bytes| MetadataMessage::from_bytes(bytes).or_else(|_| Err(())));

            if let Some(ParentInfo { tx_id, tx_type }) = parent {
                match tx_type {
                    TransactionType::TxHandshake => {
                        HandshakeRecvMetadataMessageAction::new(tx_id, nonce.clone(), result)
                            .dispatch_pure(state);
                    }
                    _ => panic!("Unhandled parent transaction type"),
                }
            }
        }
    }

    fn cancel(&self, _tx_id: u64, _parent: Option<ParentInfo>, _state: &GlobalState) {}
}

impl TxRecvMetadataMessage {
    pub fn new(token: mio::Token, nonce: Nonce, key: PrecomputedKey) -> Self {
        Self::Receiving(RecvMetadataMessageContext { token, nonce, key })
    }

    fn is_receiving(&self) -> bool {
        match self {
            Self::Receiving(_) => true,
            _ => false,
        }
    }
}

pub struct RecvMetadataMessageAction {
    tx_id: u64,
    result: Result<Vec<u8>, ()>,
}

impl RecvMetadataMessageAction {
    pub fn new(tx_id: u64, result: Result<Vec<u8>, ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for RecvMetadataMessageAction {
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = TxRecvMetadataMessage::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            assert!(tx_state.is_receiving());

            if let TxRecvMetadataMessage::Receiving(ctx) = tx_state.clone() {
                let result = self
                    .result
                    .clone()
                    .and_then(|bytes| ctx.key.decrypt(&bytes, &ctx.nonce).or_else(|_| Err(())));

                tx_state.set(TxRecvMetadataMessage::Completed(
                    ctx.nonce.increment(),
                    result,
                ));
                //drop(transaction);
                //drop(transactions);
                CompleteTransactionAction::<TxRecvMetadataMessage>::new(self.tx_id)
                    .dispatch_pure(state);
            }
        }
    }
}

#[derive(Clone)]
pub struct SendMetadataMessageContext {
    pub token: mio::Token,
    pub nonce: Nonce,
    pub bytes_to_send: Vec<u8>,
}

#[derive(Clone)]
pub enum TxSendMetadataMessage {
    Transmitting(SendMetadataMessageContext),
    Completed(Nonce, Result<(), ()>),
}

impl TransactionStage for TxSendMetadataMessage {
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

impl Transaction for TxSendMetadataMessage {
    fn init(&mut self, tx_id: u64, state: &GlobalState) {
        match self {
            TxSendMetadataMessage::Transmitting(ctx) => {
                CreateTransactionAction::new(
                    0,
                    0,
                    TxSendChunk::try_new(ctx.token, ctx.bytes_to_send.clone()).unwrap(),
                    Some(ParentInfo {
                        tx_id,
                        tx_type: TransactionType::TxSendMetadataMessage,
                    }),
                )
                .dispatch_pure(state);
            }
            _ => panic!("Invalid intitial state"),
        }
    }

    fn commit(&self, _tx_id: u64, parent: Option<ParentInfo>, state: &GlobalState) {
        assert!(self.is_completed() && parent.is_some());

        if let TxSendMetadataMessage::Completed(nonce, result) = self {
            if let Some(ParentInfo { tx_id, tx_type }) = parent {
                match tx_type {
                    TransactionType::TxHandshake => {
                        HandshakeSendMetadataMessageCompleteAction::new(
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

impl TxSendMetadataMessage {
    pub fn try_new(
        token: mio::Token,
        nonce: Nonce,
        key: PrecomputedKey,
        msg: MetadataMessage,
    ) -> Result<Self, ()> {
        let bytes = msg.as_bytes().or_else(|_| Err(()))?;
        let bytes_to_send = key.encrypt(&bytes, &nonce).or_else(|_| Err(()))?;

        Ok(Self::Transmitting(SendMetadataMessageContext {
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

pub struct SendMetadataMessageCompleteAction {
    tx_id: u64,
    result: Result<(), ()>,
}

impl SendMetadataMessageCompleteAction {
    pub fn new(tx_id: u64, result: Result<(), ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for SendMetadataMessageCompleteAction {
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = TxSendMetadataMessage::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            assert!(tx_state.is_transmitting());

            if let TxSendMetadataMessage::Transmitting(ctx) = tx_state.clone() {
                tx_state.set(TxSendMetadataMessage::Completed(
                    ctx.nonce.increment(),
                    self.result,
                ));
                //drop(transaction);
                //drop(transactions);
                CompleteTransactionAction::<TxSendMetadataMessage>::new(self.tx_id)
                    .dispatch_pure(state);
            }
        }
    }
}
