use std::mem::size_of;

use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryChunkError};

use crate::{action::PureAction, state::GlobalState};

use super::{
    data_transfer::{RecvDataResult, SendDataResult, TxRecvData, TxSendData},
    handshake::{
        ack::{RecvAckMessageAction, SendAckMessageCompleteAction},
        connection::{RecvConnectionMessageAction, SendConnectionMessageCompleteAction},
        metadata::{RecvMetadataMessageAction, SendMetadataMessageCompleteAction},
    },
    transaction::{
        CompleteTransactionAction, CreateTransactionAction, Transaction, TransactionStage,
        TransactionType,
    },
};

#[derive(PartialEq, Clone)]
pub enum RecvChunkStage {
    ReceivingHeader,
    ReceivingBody,
    Finish,
}

impl TransactionStage for RecvChunkStage {
    fn is_stage_enabled(&self, stage: &Self) -> bool {
        let mut enabled_stages = match self {
            Self::ReceivingHeader => [Self::ReceivingBody, Self::Finish].iter(),
            Self::ReceivingBody => [Self::Finish].iter(),
            Self::Finish => [].iter(),
        };

        enabled_stages.any(|s| *s == *stage)
    }
}

#[derive(Clone)]
pub struct TxRecvChunk {
    stage: RecvChunkStage,
    result: Option<RecvDataResult>,
    parent_tx: u64,
    token: mio::Token,
    chunk_len: u16,
}

impl TxRecvChunk {
    pub fn new(parent_tx: u64, token: mio::Token) -> Self {
        Self {
            stage: RecvChunkStage::ReceivingHeader,
            result: None,
            parent_tx: parent_tx,
            token: token,
            chunk_len: 0,
        }
    }

    fn init_reducer(tx_id: u64, state: &mut GlobalState) {
        let transactions = state.transactions();
        let transaction = transactions.get(&tx_id).unwrap();

        if let TransactionType::TxRecvChunk(tx_state) = transaction.tx_type() {
            let token = tx_state.token;

            drop(transactions);
            // create child transaction to receive the chunk length
            CreateTransactionAction::new(
                &[],
                &[],
                TxRecvData::new(tx_id, token, size_of::<u16>()).into(),
            )
            .dispatch_pure(state);
        } else {
            panic!("TxRecvChunk init_reducer: Invalid Transaction type")
        }
    }

    fn commit_reducer(_tx_id: u64, _state: &mut GlobalState) {}
}

impl Transaction for TxRecvChunk {
    fn init_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::init_reducer
    }

    fn commit_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::commit_reducer
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
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxRecvChunk(tx_state) => {
                match &self.result {
                    Ok(bytes_received) => {
                        match tx_state.stage {
                            RecvChunkStage::ReceivingHeader => {
                                // TODO: proper handling
                                let chunk_len = u16::from_be_bytes(
                                    bytes_received.as_slice().try_into().unwrap(),
                                );
                                assert!(chunk_len != 0);
                                tx_state.chunk_len = chunk_len;
                                tx_state.stage.set_stage(RecvChunkStage::ReceivingBody);

                                let token = tx_state.token;

                                drop(transactions);
                                // create child transaction to receive the chunk body
                                CreateTransactionAction::new(
                                    &[],
                                    &[],
                                    TxRecvData::new(self.tx_id, token, chunk_len.into()).into(),
                                )
                                .dispatch_pure(state);
                            }
                            RecvChunkStage::ReceivingBody => {
                                // TODO: proper handling
                                assert!(bytes_received.len() == tx_state.chunk_len as usize);

                                tx_state.result = Some(RecvDataResult::Success);
                                drop(transactions);
                                RecvChunkCompleteAction::new(self.tx_id, self.result.clone())
                                    .dispatch_pure(state);
                            }
                            RecvChunkStage::Finish => {
                                panic!("RecvChunkAction reducer: invalid transaction stage")
                            }
                        }
                    }
                    Err(_) => {
                        tx_state.result = Some(RecvDataResult::Error);
                        drop(transactions);
                        RecvChunkCompleteAction::new(self.tx_id, self.result.clone())
                            .dispatch_pure(state);
                    }
                }
            }
            _ => panic!("RecvChunkAction reducer: invalid transaction type"),
        }
    }
}

pub struct RecvChunkCompleteAction {
    tx_id: u64,
    result: Result<Vec<u8>, ()>,
}

impl RecvChunkCompleteAction {
    pub fn new(tx_id: u64, result: Result<Vec<u8>, ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for RecvChunkCompleteAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxRecvChunk(tx_state) => {
                let parent_tx_id = tx_state.parent_tx;

                tx_state.stage.set_stage(RecvChunkStage::Finish);
                drop(transaction);

                let parent_transaction = transactions.get_mut(&parent_tx_id).unwrap();

                match parent_transaction.tx_type_mut() {
                    TransactionType::TxRecvConnectionMessage(_) => {
                        drop(transactions);
                        RecvConnectionMessageAction::new(parent_tx_id, self.result.clone())
                            .dispatch_pure(state);
                        CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                    }
                    TransactionType::TxRecvMetadataMessage(_) => {
                        drop(transactions);
                        RecvMetadataMessageAction::new(parent_tx_id, self.result.clone())
                            .dispatch_pure(state);
                        CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                    }
                    TransactionType::TxRecvAckMessage(_) => {
                        drop(transactions);
                        RecvAckMessageAction::new(parent_tx_id, self.result.clone())
                            .dispatch_pure(state);
                        CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                    }
                    _ => panic!("RecvChunkCompleteAction reducer: unknown parent transaction type"),
                }
            }
            _ => panic!("RecvChunkCompleteAction reducer: invalid transaction type"),
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum SendChunkStage {
    Transmitting,
    Finish,
}

impl TransactionStage for SendChunkStage {
    fn is_stage_enabled(&self, stage: &Self) -> bool {
        let mut enabled_stages = match self {
            Self::Transmitting => [Self::Finish].iter(),
            Self::Finish => [].iter(),
        };

        enabled_stages.any(|s| *s == *stage)
    }
}

#[derive(Clone)]
pub struct TxSendChunk {
    stage: SendChunkStage,
    result: Option<SendDataResult>,
    parent_tx: u64,
    token: mio::Token,
    bytes_to_send: Vec<u8>,
}

impl TxSendChunk {
    pub fn try_new(
        parent_tx: u64,
        token: mio::Token,
        bytes_to_send: Vec<u8>,
    ) -> Result<Self, BinaryChunkError> {
        let chunk = BinaryChunk::from_content(bytes_to_send.as_slice())?;
        Ok(Self {
            stage: SendChunkStage::Transmitting,
            result: None,
            parent_tx,
            token,
            bytes_to_send: chunk.raw().to_vec(),
        })
    }

    fn init_reducer(tx_id: u64, state: &mut GlobalState) {
        let transactions = state.transactions();
        let transaction = transactions.get(&tx_id).unwrap();

        if let TransactionType::TxSendChunk(tx_state) = transaction.tx_type() {
            let token = tx_state.token;
            let bytes_to_send = tx_state.bytes_to_send.clone();

            drop(transactions);
            CreateTransactionAction::new(
                &[],
                &[],
                TxSendData::new(tx_id, token, bytes_to_send).into(),
            )
            .dispatch_pure(state);
        } else {
            panic!("TxSendChunk init_reducer: Invalid Transaction type")
        }
    }

    fn commit_reducer(_tx_id: u64, _state: &mut GlobalState) {}
}

impl Transaction for TxSendChunk {
    fn init_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::init_reducer
    }

    fn commit_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::commit_reducer
    }
}

pub struct SendChunkCompleteAction {
    tx_id: u64,
    result: SendDataResult,
}

impl SendChunkCompleteAction {
    pub fn new(tx_id: u64, result: SendDataResult) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for SendChunkCompleteAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxSendChunk(tx_state) => match tx_state.stage {
                SendChunkStage::Transmitting => {
                    let parent_tx_id = tx_state.parent_tx;

                    tx_state.result = Some(self.result.clone());
                    tx_state.stage.set_stage(SendChunkStage::Finish);
                    drop(transaction);

                    let parent_transaction = transactions.get_mut(&parent_tx_id).unwrap();

                    match parent_transaction.tx_type() {
                        TransactionType::TxSendConnectionMessage(_) => {
                            drop(transactions);
                            SendConnectionMessageCompleteAction::new(
                                parent_tx_id,
                                self.result.clone(),
                            )
                            .dispatch_pure(state)
                        }
                        TransactionType::TxSendMetadataMessage(_) => {
                            drop(transactions);
                            SendMetadataMessageCompleteAction::new(
                                parent_tx_id,
                                self.result.clone(),
                            )
                            .dispatch_pure(state)
                        }
                        TransactionType::TxSendAckMessage(_) => {
                            drop(transactions);
                            SendAckMessageCompleteAction::new(parent_tx_id, self.result.clone())
                                .dispatch_pure(state)
                        }
                        _ => panic!(
                            "SendChunkCompleteAction reducer: unknown parent transaction type"
                        ),
                    }

                    CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                }
                SendChunkStage::Finish => {
                    panic!("SendChunkCompleteAction reducer: called at invalid stage")
                }
            },
            _ => panic!("SendChunkCompleteAction reducer: Invalid Transaction type"),
        }
    }
}
