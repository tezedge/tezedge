use super::{
    chunk::{RecvChunkAction, SendChunkCompleteAction},
    transaction::{CompleteTransactionAction, ParentInfo, TransactionStage, TransactionType},
};
use crate::transaction::transaction::{Transaction, TransactionState, TxOps};
use crate::{action::PureAction, state::GlobalState};

#[derive(Clone)]
pub struct RecvContext {
    pub token: mio::Token,
    pub bytes_remaining: usize,
    pub bytes_received: Vec<u8>,
}

#[derive(Clone)]
pub enum TxRecvData {
    Receiving(RecvContext),
    Completed(Result<Vec<u8>, ()>),
}

impl TransactionStage for TxRecvData {
    fn is_enabled(&self, next_stage: &Self) -> bool {
        match self {
            Self::Receiving(ctx) => match next_stage {
                Self::Completed(_) => true,
                Self::Receiving(new_ctx) => {
                    ctx.token == new_ctx.token
                        && new_ctx.bytes_remaining < ctx.bytes_remaining
                        && new_ctx.bytes_remaining != 0
                }
            },
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

impl Transaction for TxRecvData {
    fn init(&mut self, _tx_id: u64, _state: &GlobalState) {}

    fn commit(&self, _tx_id: u64, parent: Option<ParentInfo>, state: &GlobalState) {
        assert!(self.is_completed() && parent.is_some());

        if let TxRecvData::Completed(result) = self {
            if let Some(ParentInfo { tx_id, tx_type }) = parent {
                match tx_type {
                    TransactionType::TxRecvChunk => {
                        RecvChunkAction::new(tx_id, result.clone()).dispatch_pure(state);
                    }
                    _ => panic!("Unhandled parent transaction type"),
                }
            }
        }
    }

    fn cancel(&self, _tx_id: u64, _parent: Option<ParentInfo>, _state: &GlobalState) {}
}

impl TxRecvData {
    pub fn new(token: mio::Token, data_len: usize) -> Self {
        Self::Receiving(RecvContext {
            token,
            bytes_remaining: data_len,
            bytes_received: Vec::new(),
        })
    }

    fn is_receiving(&self) -> bool {
        match self {
            Self::Receiving(_) => true,
            _ => false,
        }
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
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = TxRecvData::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            assert!(tx_state.is_receiving());

            if let TxRecvData::Receiving(mut ctx) = tx_state.clone() {
                if let Ok(data) = &self.result {
                    assert!(data.len() <= ctx.bytes_remaining);
                    ctx.bytes_remaining -= data.len();
                    ctx.bytes_received.extend(data);

                    if ctx.bytes_remaining == 0 {
                        tx_state.set(TxRecvData::Completed(Ok(ctx.bytes_received)));
                        //drop(transaction);
                        //drop(transactions);
                        CompleteTransactionAction::<TxRecvData>::new(self.tx_id)
                            .dispatch_pure(state);
                    } else {
                        // context update
                        tx_state.set(TxRecvData::Receiving(ctx.clone()));
                    }
                } else {
                    tx_state.set(TxRecvData::Completed(Err(())));
                    //drop(transaction);
                    //drop(transactions);
                    CompleteTransactionAction::<TxRecvData>::new(self.tx_id).dispatch_pure(state);
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct SendContext {
    pub token: mio::Token,
    pub bytes_to_send: Vec<u8>,
}

#[derive(Clone)]
pub enum TxSendData {
    Transmitting(SendContext),
    Completed(Result<(), ()>),
}

impl TransactionStage for TxSendData {
    fn is_enabled(&self, next_stage: &Self) -> bool {
        match self {
            Self::Transmitting(ctx) => match next_stage {
                Self::Completed(_) => true,
                Self::Transmitting(new_ctx) => {
                    ctx.token == new_ctx.token
                        && new_ctx.bytes_to_send.len() < ctx.bytes_to_send.len()
                }
            },
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

impl Transaction for TxSendData {
    fn init(&mut self, _tx_id: u64, _state: &GlobalState) {}

    fn commit(&self, _tx_id: u64, parent: Option<ParentInfo>, state: &GlobalState) {
        assert!(self.is_completed() && parent.is_some());

        if let TxSendData::Completed(result) = *self {
            if let Some(ParentInfo { tx_id, tx_type }) = parent {
                match tx_type {
                    TransactionType::TxSendChunk => {
                        SendChunkCompleteAction::new(tx_id, result).dispatch_pure(state);
                    }
                    _ => panic!("Unhandled parent transaction type"),
                }
            }
        }
    }

    fn cancel(&self, _tx_id: u64, _parent: Option<ParentInfo>, _state: &GlobalState) {}
}

impl TxSendData {
    pub fn new(token: mio::Token, bytes_to_send: Vec<u8>) -> Self {
        Self::Transmitting(SendContext {
            token,
            bytes_to_send,
        })
    }

    fn is_transmitting(&self) -> bool {
        match self {
            Self::Transmitting(_) => true,
            _ => false,
        }
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
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = TxSendData::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            assert!(tx_state.is_transmitting());

            if let TxSendData::Transmitting(mut ctx) = tx_state.clone() {
                if let Ok(bytes_sent) = self.result {
                    assert!(bytes_sent <= ctx.bytes_to_send.len());
                    ctx.bytes_to_send = ctx.bytes_to_send[bytes_sent..].to_vec();

                    if ctx.bytes_to_send.len() == 0 {
                        tx_state.set(TxSendData::Completed(Ok(())));
                        //drop(transaction);
                        //drop(transactions);
                        CompleteTransactionAction::<TxSendData>::new(self.tx_id)
                            .dispatch_pure(state);
                    } else {
                        // context update
                        tx_state.set(TxSendData::Transmitting(ctx));
                    }
                } else {
                    tx_state.set(TxSendData::Completed(Err(())));
                    //drop(transaction);
                    //drop(transactions);
                    CompleteTransactionAction::<TxSendData>::new(self.tx_id).dispatch_pure(state);
                }
            }
        }
    }
}
