use super::{
    chunk::{RecvChunkAction, SendChunkCompleteAction},
    transaction::{CompleteTransactionAction, TransactionStage},
};
use crate::transaction::transaction::{Transaction, TransactionType};
use crate::{action::PureAction, state::GlobalState};
#[derive(Clone)]
pub enum RecvDataResult {
    Success,
    Error,
}

#[derive(PartialEq, Clone)]
pub enum RecvDataStage {
    Receiving,
    Finish,
}

impl TransactionStage for RecvDataStage {
    fn is_stage_enabled(&self, stage: &Self) -> bool {
        let mut enabled_stages = match self {
            Self::Receiving => [Self::Finish].iter(),
            Self::Finish => [].iter(),
        };

        enabled_stages.any(|s| *s == *stage)
    }
}

#[derive(Clone)]
pub struct TxRecvData {
    stage: RecvDataStage,
    result: Option<RecvDataResult>,
    parent_tx: u64,
    token: mio::Token,
    bytes_remaining: usize,
    bytes_received: Vec<u8>,
}

impl Transaction for TxRecvData {
    fn init_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::init_reducer
    }

    fn commit_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::commit_reducer
    }
}

impl TxRecvData {
    pub fn new(parent_tx: u64, token: mio::Token, data_len: usize) -> Self {
        Self {
            stage: RecvDataStage::Receiving,
            result: None,
            parent_tx: parent_tx,
            token: token,
            bytes_remaining: data_len,
            bytes_received: Vec::new(),
        }
    }

    fn init_reducer(_tx_id: u64, _state: &mut GlobalState) {
        //state.get_tx_mut(tx_id).set_pending_effects(true);
    }

    fn commit_reducer(_tx_id: u64, _state: &mut GlobalState) {}

    pub fn token(&self) -> &mio::Token {
        &self.token
    }

    pub fn stage(&self) -> RecvDataStage {
        self.stage.clone()
    }

    pub fn bytes_remaining(&self) -> usize {
        self.bytes_remaining
    }
}

pub struct RecvDataAction {
    tx_id: u64,
    result: Result<Vec<u8>, ()>,
}

impl RecvDataAction {
    pub fn new(tx_id: u64, result: Result<Vec<u8>, ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for RecvDataAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxRecvData(tx_state) => {
                match &self.result {
                    Ok(bytes_received) => {
                        assert!(bytes_received.len() <= tx_state.bytes_remaining);
                        tx_state.bytes_remaining -= bytes_received.len();
                        tx_state.bytes_received.extend(bytes_received);

                        if tx_state.bytes_remaining == 0 {
                            tx_state.result = Some(RecvDataResult::Success);
                        }
                    }
                    Err(_) => tx_state.result = Some(RecvDataResult::Error),
                }

                if tx_state.result.is_some() {
                    let result = match tx_state.result.as_ref().unwrap() {
                        RecvDataResult::Success => Ok(tx_state.bytes_received.clone()),
                        RecvDataResult::Error => Err(()),
                    };

                    drop(transactions);
                    RecvDataCompleteAction::new(self.tx_id, result).dispatch_pure(state);
                }
            }
            _ => panic!("RecvDataAction reducer: Invalid Transaction type"),
        }
    }
}

pub struct RecvDataCompleteAction {
    tx_id: u64,
    result: Result<Vec<u8>, ()>,
}

impl RecvDataCompleteAction {
    pub fn new(tx_id: u64, result: Result<Vec<u8>, ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for RecvDataCompleteAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxRecvData(tx_state) => {
                tx_state.stage.set_stage(RecvDataStage::Finish);

                let parent_tx_id = tx_state.parent_tx;
                let parent_transaction = transactions.get(&parent_tx_id).unwrap();

                match parent_transaction.tx_type() {
                    TransactionType::TxRecvChunk(_) => {
                        drop(transactions);
                        RecvChunkAction::new(parent_tx_id, self.result.clone())
                            .dispatch_pure(state);
                        CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                    }
                    _ => panic!("RecvDataCompleteAction reducer: unknown parent transaction type"),
                }
            }
            _ => panic!("RecvDataCompleteAction reducer: Invalid Transaction type"),
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum SendDataResult {
    Success,
    Error,
}

#[derive(PartialEq, Clone)]
pub enum SendDataStage {
    Transmitting,
    Finish,
}

impl TransactionStage for SendDataStage {
    fn is_stage_enabled(&self, stage: &Self) -> bool {
        let mut enabled_stages = match self {
            Self::Transmitting => [Self::Finish].iter(),
            Self::Finish => [].iter(),
        };

        enabled_stages.any(|s| *s == *stage)
    }
}

#[derive(Clone)]
pub struct TxSendData {
    stage: SendDataStage,
    result: Option<SendDataResult>,
    parent_tx: u64,
    token: mio::Token,
    bytes_to_send: Vec<u8>,
}

impl TxSendData {
    pub fn new(parent_tx: u64, token: mio::Token, bytes_to_send: Vec<u8>) -> Self {
        Self {
            stage: SendDataStage::Transmitting,
            result: None,
            parent_tx,
            token,
            bytes_to_send,
        }
    }

    pub fn token(&self) -> &mio::Token {
        &self.token
    }

    pub fn stage(&self) -> SendDataStage {
        self.stage.clone()
    }

    pub fn bytes_to_send(&self) -> &Vec<u8> {
        &self.bytes_to_send
    }

    fn init_reducer(_tx_id: u64, _state: &mut GlobalState) {
        //state.get_tx_mut(tx_id).set_pending_effects(true);
    }

    fn commit_reducer(_tx_id: u64, _state: &mut GlobalState) {}

}

impl Transaction for TxSendData {
    fn init_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::init_reducer
    }

    fn commit_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::commit_reducer
    }
}

pub struct SendDataAction {
    tx_id: u64,
    result: Result<usize, ()>,
}

impl SendDataAction {
    pub fn new(tx_id: u64, result: Result<usize, ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for SendDataAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        if let TransactionType::TxSendData(tx_state) = transaction.tx_type_mut() {
            match self.result {
                Ok(bytes_sent) => {
                    assert!(bytes_sent <= tx_state.bytes_to_send.len());
                    tx_state.bytes_to_send = tx_state.bytes_to_send[bytes_sent..].to_vec();

                    // no more bytes remaining, progress to finish stage
                    if tx_state.bytes_to_send.len() == 0 {
                        tx_state.result = Some(SendDataResult::Success);
                    }
                }
                Err(_) => tx_state.result = Some(SendDataResult::Error),
            }

            if tx_state.result.is_some() {
                let result = tx_state.result.clone().unwrap();

                drop(transactions);
                SendDataCompleteAction::new(self.tx_id, result).dispatch_pure(state);
            }
        } else {
            panic!("SendDataAction reducer: invalid transaction type");
        }
    }
}

pub struct SendDataCompleteAction {
    tx_id: u64,
    result: SendDataResult,
}

impl SendDataCompleteAction {
    pub fn new(tx_id: u64, result: SendDataResult) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for SendDataCompleteAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        if let TransactionType::TxSendData(tx_state) = transaction.tx_type_mut() {
            let parent_tx_id = tx_state.parent_tx;
            tx_state.stage.set_stage(SendDataStage::Finish);
            drop(transaction);

            let parent_transaction = transactions.get(&parent_tx_id).unwrap();

            match parent_transaction.tx_type() {
                TransactionType::TxSendChunk(_) => {
                    drop(transactions);
                    SendChunkCompleteAction::new(parent_tx_id, self.result.clone())
                        .dispatch_pure(state);
                    CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                }
                _ => panic!("SendDataCompleteAction reducer: unknown parent transaction type"),
            }
        } else {
            panic!("SendDataCompleteAction reducer: Invalid Transaction type")
        }
    }
}
