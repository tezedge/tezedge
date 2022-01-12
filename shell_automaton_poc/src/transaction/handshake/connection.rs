use tezos_messages::p2p::{
    binary_message::{BinaryRead, BinaryWrite},
    encoding::connection::ConnectionMessage,
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
    HandshakeRecvConnectionMessageAction, HandshakeSendConnectionMessageCompleteAction,
};

#[derive(PartialEq, Clone)]
pub enum RecvConnectionMessageStage {
    Receiving,
    Finish,
}

impl TransactionStage for RecvConnectionMessageStage {
    fn is_stage_enabled(&self, stage: &Self) -> bool {
        let mut enabled_stages = match self {
            Self::Receiving => [Self::Finish].iter(),
            Self::Finish => [].iter(),
        };

        enabled_stages.any(|s| *s == *stage)
    }
}

#[derive(Clone)]
pub struct TxRecvConnectionMessage {
    stage: RecvConnectionMessageStage,
    result: Option<RecvDataResult>,
    parent_tx: u64,
    token: mio::Token,
}

impl TxRecvConnectionMessage {
    pub fn new(parent_tx: u64, token: mio::Token) -> Self {
        Self {
            stage: RecvConnectionMessageStage::Receiving,
            result: None,
            parent_tx: parent_tx,
            token: token,
        }
    }

    fn init_reducer(tx_id: u64, state: &mut GlobalState) {
        let transactions = state.transactions();
        let transaction = transactions.get(&tx_id).unwrap();

        if let TransactionType::TxRecvConnectionMessage(tx_state) = transaction.tx_type() {
            let token = tx_state.token.clone();
            drop(transactions);
            // message fits in a single chunk
            CreateTransactionAction::new(&[], &[], TxRecvChunk::new(tx_id, token).into())
                .dispatch_pure(state);
        } else {
            panic!("TxRecvConnectionMessage init_reducer: Invalid Transaction type")
        }
    }

    fn commit_reducer(_tx_id: u64, _state: &mut GlobalState) {}
}

impl Transaction for TxRecvConnectionMessage {
    fn init_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::init_reducer
    }

    fn commit_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::commit_reducer
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
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxRecvConnectionMessage(_) => {
                let result = match &self.result {
                    Ok(bytes) => match ConnectionMessage::from_bytes(bytes) {
                        Ok(msg) => Ok(msg),
                        _ => Err(()),
                    },
                    _ => Err(()),
                };
                drop(transactions);
                RecvConnectionMessageCompleteAction::new(self.tx_id, result).dispatch_pure(state)
            }
            _ => panic!("RecvConnectionMessageAction reducer: Invalid Transaction type"),
        }
    }
}

pub struct RecvConnectionMessageCompleteAction {
    tx_id: u64,
    result: Result<ConnectionMessage, ()>,
}

impl RecvConnectionMessageCompleteAction {
    pub fn new(tx_id: u64, result: Result<ConnectionMessage, ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for RecvConnectionMessageCompleteAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxRecvConnectionMessage(tx_state) => {
                let parent_tx_id = tx_state.parent_tx;
                let result = match self.result {
                    Ok(_) => RecvDataResult::Success,
                    _ => RecvDataResult::Error,
                };
                tx_state.result = Some(result);
                tx_state.stage.set_stage(RecvConnectionMessageStage::Finish);

                let parent_transaction = transactions.get(&parent_tx_id).unwrap();

                match parent_transaction.tx_type() {
                    TransactionType::TxHandshake(_) => {
                        drop(transactions);
                        HandshakeRecvConnectionMessageAction::new(parent_tx_id, self.result.clone()).dispatch_pure(state);
                    },
                    _ => panic!("RecvConnectionMessageCompleteAction reducer: unknown parent transaction type")
                }

                CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
            }
            _ => panic!("RecvConnectionMessageCompleteAction reducer: Invalid Transaction type"),
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum SendConnectionMessageStage {
    Transmitting,
    Finish,
}

impl TransactionStage for SendConnectionMessageStage {
    fn is_stage_enabled(&self, stage: &Self) -> bool {
        let mut enabled_stages = match self {
            Self::Transmitting => [Self::Finish].iter(),
            Self::Finish => [].iter(),
        };

        enabled_stages.any(|s| *s == *stage)
    }
}

#[derive(Clone)]
pub struct TxSendConnectionMessage {
    stage: SendConnectionMessageStage,
    result: Option<SendDataResult>,
    parent_tx: u64,
    token: mio::Token,
    bytes_to_send: Vec<u8>,
}

impl TxSendConnectionMessage {
    pub fn try_new(parent_tx: u64, token: mio::Token, msg: ConnectionMessage) -> Result<Self, ()> {
        if let Ok(bytes_to_send) = msg.as_bytes() {
            return Ok(Self {
                stage: SendConnectionMessageStage::Transmitting,
                result: None,
                parent_tx,
                token,
                bytes_to_send,
            });
        }

        return Err(());
    }

    fn init_reducer(tx_id: u64, state: &mut GlobalState) {
        let transactions = state.transactions();
        let transaction = transactions.get(&tx_id).unwrap();

        if let TransactionType::TxSendConnectionMessage(tx_state) = transaction.tx_type() {
            let token = tx_state.token.clone();
            let bytes_to_send = tx_state.bytes_to_send.clone();

            drop(transactions);
            // TODO: handle errors properly
            let send_chunk = TxSendChunk::try_new(tx_id, token, bytes_to_send).unwrap();
            // ConnectionMessage fits in a single chunk
            CreateTransactionAction::new(&[], &[], send_chunk.into()).dispatch_pure(state);
        } else {
            panic!("TxSendConnectionMessage init_reducer: Invalid Transaction type")
        }
    }

    fn commit_reducer(_tx_id: u64, _state: &mut GlobalState) {}
}

impl Transaction for TxSendConnectionMessage {
    fn init_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::init_reducer
    }

    fn commit_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::commit_reducer
    }
}

pub struct SendConnectionMessageCompleteAction {
    tx_id: u64,
    result: SendDataResult,
}

impl SendConnectionMessageCompleteAction {
    pub fn new(tx_id: u64, result: SendDataResult) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for SendConnectionMessageCompleteAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxSendConnectionMessage(tx_state) => match tx_state.stage {
                SendConnectionMessageStage::Transmitting => {
                    let parent_tx_id = tx_state.parent_tx;
                    tx_state.result = Some(self.result.clone());
                    tx_state.stage.set_stage(SendConnectionMessageStage::Finish);

                    let parent_transaction = transactions.get(&parent_tx_id).unwrap();

                    match parent_transaction.tx_type() {
                            TransactionType::TxHandshake(_) => {
                                drop(transactions);
                                HandshakeSendConnectionMessageCompleteAction::new(parent_tx_id, self.result.clone())
                                .dispatch_pure(state)
                            },
                            _ => panic!("SendConnectionMessageCompleteAction reducer: unknown parent transaction type")
                        }

                    CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                }
                SendConnectionMessageStage::Finish => {
                    panic!("SendConnectionMessageCompleteAction reducer: called at invalid stage")
                }
            },
            _ => panic!("SendConnectionMessageCompleteAction reducer: Invalid Transaction type"),
        }
    }
}
