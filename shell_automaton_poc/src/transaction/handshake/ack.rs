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
        data_transfer::{RecvDataResult, SendDataResult},
        transaction::{
            CompleteTransactionAction, CreateTransactionAction, Transaction, TransactionStage,
            TransactionType,
        },
    },
};

use super::handshake::{HandshakeRecvAckMessageAction, HandshakeSendAckMessageCompleteAction};

#[derive(PartialEq, Clone)]
pub enum SendAckMessageStage {
    Transmitting,
    Finish,
}

impl TransactionStage for SendAckMessageStage {
    fn is_stage_enabled(&self, stage: &Self) -> bool {
        let mut enabled_stages = match self {
            Self::Transmitting => [Self::Finish].iter(),
            Self::Finish => [].iter(),
        };

        enabled_stages.any(|s| *s == *stage)
    }
}

#[derive(Clone)]
pub struct TxSendAckMessage {
    stage: SendAckMessageStage,
    result: Option<SendDataResult>,
    parent_tx: u64,
    token: mio::Token,
    nonce: Nonce,
    bytes_to_send: Vec<u8>,
}

impl TxSendAckMessage {
    pub fn try_new(
        parent_tx: u64,
        token: mio::Token,
        nonce: Nonce,
        key: PrecomputedKey,
        msg: AckMessage,
    ) -> Result<Self, ()> {
        if let Ok(bytes) = msg.as_bytes() {
            if let Ok(bytes_to_send) = key.encrypt(&bytes, &nonce) {
                return Ok(Self {
                    stage: SendAckMessageStage::Transmitting,
                    result: None,
                    parent_tx,
                    token,
                    nonce: nonce.increment(),
                    bytes_to_send,
                });
            }
        }

        return Err(());
    }

    fn init_reducer(tx_id: u64, state: &mut GlobalState) {
        let transactions = state.transactions();
        let transaction = transactions.get(&tx_id).unwrap();

        if let TransactionType::TxSendAckMessage(tx_state) = transaction.tx_type() {
            // TODO: handle errors properly
            let send_chunk =
                TxSendChunk::try_new(tx_id, tx_state.token, tx_state.bytes_to_send.clone())
                    .unwrap();
            drop(transactions);
            // AckMessage fits in a single chunk
            CreateTransactionAction::new(&[], &[], send_chunk.into()).dispatch_pure(state);
        } else {
            panic!("TxSendAckMessage init_reducer: Invalid Transaction type")
        }
    }

    fn commit_reducer(_tx_id: u64, _state: &mut GlobalState) {}
}

impl Transaction for TxSendAckMessage {
    fn init_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::init_reducer
    }

    fn commit_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::commit_reducer
    }
}

pub struct SendAckMessageCompleteAction {
    tx_id: u64,
    result: SendDataResult,
}

impl SendAckMessageCompleteAction {
    pub fn new(tx_id: u64, result: SendDataResult) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for SendAckMessageCompleteAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxSendAckMessage(tx_state) => {
                match tx_state.stage {
                    SendAckMessageStage::Transmitting => {
                        let parent_tx_id = tx_state.parent_tx;
                        let nonce = tx_state.nonce.clone();

                        tx_state.result = Some(self.result.clone());
                        tx_state.stage.set_stage(SendAckMessageStage::Finish);

                        drop(transaction);

                        let parent_transaction = transactions.get(&parent_tx_id).unwrap();

                        match parent_transaction.tx_type() {
                            TransactionType::TxHandshake(_) => {
                                drop(transactions);
                                HandshakeSendAckMessageCompleteAction::new(
                                    parent_tx_id,
                                    nonce,
                                    self.result.clone()
                                ).dispatch_pure(state);
                                CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                            },
                            _ => panic!("SendAckMessageCompleteAction reducer: unknown parent transaction type")
                        }
                    }
                    SendAckMessageStage::Finish => {
                        panic!("SendAckMessageCompleteAction reducer: called at invalid stage")
                    }
                }
            }
            _ => panic!("SendAckMessageCompleteAction reducer: Invalid Transaction type"),
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum RecvAckMessageStage {
    Receiving,
    Finish,
}

impl TransactionStage for RecvAckMessageStage {
    fn is_stage_enabled(&self, stage: &Self) -> bool {
        let mut enabled_stages = match self {
            Self::Receiving => [Self::Finish].iter(),
            Self::Finish => [].iter(),
        };

        enabled_stages.any(|s| *s == *stage)
    }
}

#[derive(Clone)]
pub struct TxRecvAckMessage {
    stage: RecvAckMessageStage,
    result: Option<RecvDataResult>,
    parent_tx: u64,
    token: mio::Token,
    nonce: Nonce,
    key: PrecomputedKey,
}

impl TxRecvAckMessage {
    pub fn new(parent_tx: u64, token: mio::Token, nonce: Nonce, key: PrecomputedKey) -> Self {
        Self {
            stage: RecvAckMessageStage::Receiving,
            result: None,
            parent_tx,
            token,
            nonce,
            key,
        }
    }

    fn init_reducer(tx_id: u64, state: &mut GlobalState) {
        let transactions = state.transactions();
        let transaction = transactions.get(&tx_id).unwrap();

        if let TransactionType::TxRecvAckMessage(tx_state) = transaction.tx_type() {
            let token = tx_state.token;

            drop(transactions);

            // message fits in a single chunk
            CreateTransactionAction::new(&[], &[], TxRecvChunk::new(tx_id, token).into())
                .dispatch_pure(state);
        } else {
            panic!("TxRecvAckMessage init_reducer: Invalid Transaction type")
        }
    }

    fn commit_reducer(_tx_id: u64, _state: &mut GlobalState) {}
}

impl Transaction for TxRecvAckMessage {
    fn init_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::init_reducer
    }

    fn commit_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::commit_reducer
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
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxRecvAckMessage(tx_state) => {
                let mut result = Err(());

                if let Ok(bytes) = &self.result {
                    if let Ok(decrypted) = tx_state.key.decrypt(&bytes, &tx_state.nonce) {
                        tx_state.nonce = tx_state.nonce.increment();

                        if let Ok(msg) = AckMessage::from_bytes(decrypted) {
                            result = Ok(msg);
                        }
                    }
                }
                drop(transactions);
                RecvAckMessageCompleteAction::new(self.tx_id, result).dispatch_pure(state)
            }
            _ => panic!("RecvAckMessageAction reducer: Invalid Transaction type"),
        }
    }
}

pub struct RecvAckMessageCompleteAction {
    tx_id: u64,
    result: Result<AckMessage, ()>,
}

impl RecvAckMessageCompleteAction {
    pub fn new(tx_id: u64, result: Result<AckMessage, ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for RecvAckMessageCompleteAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxRecvAckMessage(tx_state) => {
                let parent_tx_id = tx_state.parent_tx;
                let nonce = tx_state.nonce.clone();
                let result = match self.result {
                    Ok(_) => RecvDataResult::Success,
                    _ => RecvDataResult::Error,
                };
                tx_state.result = Some(result);
                tx_state.stage.set_stage(RecvAckMessageStage::Finish);
                drop(transaction);

                let parent_transaction = transactions.get(&parent_tx_id).unwrap();

                match parent_transaction.tx_type() {
                    TransactionType::TxHandshake(_) => {
                        drop(transactions);
                        HandshakeRecvAckMessageAction::new(
                            parent_tx_id,
                            nonce,
                            self.result.clone(),
                        )
                        .dispatch_pure(state);
                    }
                    _ => panic!(
                        "RecvAckMessageCompleteAction reducer: unknown parent transaction type"
                    ),
                }

                CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
            }
            _ => panic!("RecvAckMessageCompleteAction reducer: Invalid Transaction type"),
        }
    }
}
