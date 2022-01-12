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
        data_transfer::{RecvDataResult, SendDataResult},
        transaction::{
            CompleteTransactionAction, CreateTransactionAction, Transaction, TransactionStage,
            TransactionType,
        },
    },
};

use super::handshake::{
    HandshakeRecvMetadataMessageAction, HandshakeSendMetadataMessageCompleteAction,
};

#[derive(PartialEq, Clone)]
pub enum SendMetadataMessageStage {
    Transmitting,
    Finish,
}

impl TransactionStage for SendMetadataMessageStage {
    fn is_stage_enabled(&self, stage: &Self) -> bool {
        let mut enabled_stages = match self {
            Self::Transmitting => [Self::Finish].iter(),
            Self::Finish => [].iter(),
        };

        enabled_stages.any(|s| *s == *stage)
    }
}

#[derive(Clone)]
pub struct TxSendMetadataMessage {
    stage: SendMetadataMessageStage,
    result: Option<SendDataResult>,
    parent_tx: u64,
    token: mio::Token,
    nonce: Nonce,
    bytes_to_send: Vec<u8>,
}

impl TxSendMetadataMessage {
    pub fn try_new(
        parent_tx: u64,
        token: mio::Token,
        nonce: Nonce,
        key: PrecomputedKey,
        msg: MetadataMessage,
    ) -> Result<Self, ()> {
        if let Ok(bytes) = msg.as_bytes() {
            if let Ok(bytes_to_send) = key.encrypt(&bytes, &nonce) {
                return Ok(Self {
                    stage: SendMetadataMessageStage::Transmitting,
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

        if let TransactionType::TxSendMetadataMessage(tx_state) = transaction.tx_type() {
            // TODO: handle errors properly
            let send_chunk =
                TxSendChunk::try_new(tx_id, tx_state.token, tx_state.bytes_to_send.clone())
                    .unwrap();
            // MetadataMessage fits in a single chunk
            drop(transactions);
            CreateTransactionAction::new(&[], &[], send_chunk.into()).dispatch_pure(state);
        } else {
            panic!("TxSendMetadataMessage init_reducer: Invalid Transaction type")
        }
    }

    fn commit_reducer(_tx_id: u64, _state: &mut GlobalState) {}
}

impl Transaction for TxSendMetadataMessage {
    fn init_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::init_reducer
    }

    fn commit_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::commit_reducer
    }
}

pub struct SendMetadataMessageCompleteAction {
    tx_id: u64,
    result: SendDataResult,
}

impl SendMetadataMessageCompleteAction {
    pub fn new(tx_id: u64, result: SendDataResult) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for SendMetadataMessageCompleteAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxSendMetadataMessage(tx_state) => match tx_state.stage {
                SendMetadataMessageStage::Transmitting => {
                    let parent_tx_id = tx_state.parent_tx;
                    let nonce = tx_state.nonce.clone();
                    tx_state.result = Some(self.result.clone());
                    tx_state.stage.set_stage(SendMetadataMessageStage::Finish);

                    let parent_transaction = transactions.get(&parent_tx_id).unwrap();

                    match parent_transaction.tx_type() {
                            TransactionType::TxHandshake(_) => {
                                drop(transactions);
                                HandshakeSendMetadataMessageCompleteAction::new(
                                    parent_tx_id,
                                    nonce,
                                    self.result.clone()
                                ).dispatch_pure(state);
                                CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                            },
                            _ => panic!("SendMetadataMessageCompleteAction reducer: unknown parent transaction type")
                        }
                }
                SendMetadataMessageStage::Finish => {
                    panic!("SendMetadataMessageCompleteAction reducer: called at invalid stage")
                }
            },
            _ => panic!("SendMetadataMessageCompleteAction reducer: Invalid Transaction type"),
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum RecvMetadataMessageStage {
    Receiving,
    Finish,
}

impl TransactionStage for RecvMetadataMessageStage {
    fn is_stage_enabled(&self, stage: &Self) -> bool {
        let mut enabled_stages = match self {
            Self::Receiving => [Self::Finish].iter(),
            Self::Finish => [].iter(),
        };

        enabled_stages.any(|s| *s == *stage)
    }
}

#[derive(Clone)]
pub struct TxRecvMetadataMessage {
    stage: RecvMetadataMessageStage,
    result: Option<RecvDataResult>,
    parent_tx: u64,
    token: mio::Token,
    nonce: Nonce,
    key: PrecomputedKey,
}

impl TxRecvMetadataMessage {
    pub fn new(parent_tx: u64, token: mio::Token, nonce: Nonce, key: PrecomputedKey) -> Self {
        Self {
            stage: RecvMetadataMessageStage::Receiving,
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

        if let TransactionType::TxRecvMetadataMessage(tx_state) = transaction.tx_type() {
            let token = tx_state.token.clone();

            drop(transactions);
            // message fits in a single chunk
            CreateTransactionAction::new(&[], &[], TxRecvChunk::new(tx_id, token).into())
                .dispatch_pure(state);
        } else {
            panic!("TxRecvMetadataMessage init_reducer: Invalid Transaction type")
        }
    }

    fn commit_reducer(_tx_id: u64, _state: &mut GlobalState) {}
}

impl Transaction for TxRecvMetadataMessage {
    fn init_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::init_reducer
    }

    fn commit_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::commit_reducer
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
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxRecvMetadataMessage(tx_state) => {
                let mut result = Err(());

                if let Ok(bytes) = &self.result {
                    if let Ok(decrypted) = tx_state.key.decrypt(&bytes, &tx_state.nonce) {
                        tx_state.nonce = tx_state.nonce.increment();

                        if let Ok(msg) = MetadataMessage::from_bytes(decrypted) {
                            result = Ok(msg);
                        }
                    }
                }
                drop(transactions);
                RecvMetadataMessageCompleteAction::new(self.tx_id, result).dispatch_pure(state)
            }
            _ => panic!("RecvMetadataMessageAction reducer: Invalid Transaction type"),
        }
    }
}

pub struct RecvMetadataMessageCompleteAction {
    tx_id: u64,
    result: Result<MetadataMessage, ()>,
}

impl RecvMetadataMessageCompleteAction {
    pub fn new(tx_id: u64, result: Result<MetadataMessage, ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for RecvMetadataMessageCompleteAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxRecvMetadataMessage(tx_state) => {
                let parent_tx_id = tx_state.parent_tx;
                let nonce = tx_state.nonce.clone();
                let result = match self.result {
                    Ok(_) => RecvDataResult::Success,
                    _ => RecvDataResult::Error,
                };
                tx_state.result = Some(result);
                tx_state.stage.set_stage(RecvMetadataMessageStage::Finish);

                let parent_transaction = transactions.get(&parent_tx_id).unwrap();

                match parent_transaction.tx_type() {
                    TransactionType::TxHandshake(_) => {
                        drop(transactions);
                        HandshakeRecvMetadataMessageAction::new(parent_tx_id, nonce, self.result.clone()).dispatch_pure(state);
                    },
                    _ => panic!("RecvMetadataMessageCompleteAction reducer: unknown parent transaction type")
                }

                CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
            }
            _ => panic!("RecvMetadataMessageCompleteAction reducer: Invalid Transaction type"),
        }
    }
}
