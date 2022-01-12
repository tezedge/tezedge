use std::net::SocketAddr;

use crypto::{
    crypto_box::{CryptoKey, PrecomputedKey, PublicKey},
    nonce::{generate_nonces, Nonce, NoncePair},
};
use tezos_messages::p2p::{
    binary_message::{BinaryChunk, BinaryWrite},
    encoding::{
        ack::{AckMessage, NackInfo, NackMotive},
        connection::ConnectionMessage,
        metadata::MetadataMessage,
    },
};

use crate::{
    action::PureAction,
    state::{GlobalState, Peer},
    transaction::{
        data_transfer::SendDataResult,
        transaction::{
            CompleteTransactionAction, CreateTransactionAction, Transaction, TransactionStage,
            TransactionType,
        },
    },
};

use super::{
    ack::{TxRecvAckMessage, TxSendAckMessage},
    connection::{TxRecvConnectionMessage, TxSendConnectionMessage},
    metadata::{TxRecvMetadataMessage, TxSendMetadataMessage},
};

#[derive(PartialEq, Clone)]
pub enum HandshakeStage {
    SendConnection,
    RecvConnection,
    SendMetadata,
    RecvMetadata,
    SendAck,
    RecvAck,
    Finish,
}

impl TransactionStage for HandshakeStage {
    fn is_stage_enabled(&self, stage: &Self) -> bool {
        let mut enabled_stages = match self {
            Self::SendConnection => [Self::RecvConnection, Self::Finish].iter(),
            Self::RecvConnection => [Self::SendMetadata, Self::Finish].iter(),
            Self::SendMetadata => [Self::RecvMetadata, Self::Finish].iter(),
            Self::RecvMetadata => [Self::SendAck, Self::Finish].iter(),
            Self::SendAck => [Self::RecvAck, Self::Finish].iter(),
            Self::RecvAck => [Self::Finish].iter(),
            Self::Finish => [].iter(),
        };

        enabled_stages.any(|s| *s == *stage)
    }
}

#[derive(PartialEq, Clone)]
pub enum HandshakeResult {
    Graylisted,
    SendConnectionError,
    RecvConnectionError,
    BadPow,
    NoncePairError,
    PublicKeyError,
    ConnectSelfError,
    SendMetadataError,
    RecvMetadataError,
    SendAckError,
    RecvAckError,
    Nack(Option<NackInfo>),
    Success,
}

#[derive(Clone)]

pub struct TxHandshake {
    stage: HandshakeStage,
    result: Option<HandshakeResult>,
    token: mio::Token,
    address: SocketAddr,
    incoming: bool,
    nonce: Nonce,
    local_chunk: Option<BinaryChunk>,
    nonce_pair: Option<NoncePair>,
    pub_key: Option<PublicKey>,
    precomputed_key: Option<PrecomputedKey>,
    nack_motive: Option<NackMotive>,
}

impl TxHandshake {
    pub fn new(token: mio::Token, address: SocketAddr, incoming: bool, nonce: Nonce) -> Self {
        TxHandshake {
            stage: HandshakeStage::SendConnection,
            result: None,
            token,
            address,
            incoming,
            nonce,
            local_chunk: None,
            nonce_pair: None,
            pub_key: None,
            precomputed_key: None,
            nack_motive: None,
        }
    }

    pub fn token(&self) -> &mio::Token {
        &self.token
    }

    pub fn check_pow(&mut self, msg: &ConnectionMessage, pow_target: f64) -> Result<(), ()> {
        let pow_data = [msg.public_key.clone(), msg.proof_of_work_stamp.clone()].concat();

        match crypto::proof_of_work::check_proof_of_work(pow_data.as_slice(), pow_target) {
            Ok(_) => Ok(()),
            _ => {
                self.result = Some(HandshakeResult::BadPow);
                Err(())
            }
        }
    }

    pub fn set_nonce_pair(&mut self, msg: &ConnectionMessage) -> Result<(), ()> {
        match generate_nonces(
            self.local_chunk.as_ref().unwrap().raw(),
            BinaryChunk::from_content(&msg.as_bytes().unwrap())
                .unwrap()
                .raw(),
            self.incoming,
        ) {
            Ok(nonce_pair) => {
                self.nonce_pair = Some(nonce_pair);
                Ok(())
            }
            _ => {
                self.result = Some(HandshakeResult::NoncePairError);
                Err(())
            }
        }
    }

    pub fn set_pub_key(&mut self, msg: &ConnectionMessage) -> Result<(), ()> {
        match PublicKey::from_bytes(&msg.public_key) {
            Ok(key) => {
                self.pub_key = Some(key);
                Ok(())
            }
            _ => {
                self.result = Some(HandshakeResult::PublicKeyError);
                Err(())
            }
        }
    }

    pub fn local_nonce(&mut self) -> &Nonce {
        &self.nonce_pair.as_ref().unwrap().local
    }

    pub fn local_nonce_mut(&mut self) -> &mut Nonce {
        &mut self.nonce_pair.as_mut().unwrap().local
    }

    pub fn remote_nonce(&mut self) -> &Nonce {
        &self.nonce_pair.as_ref().unwrap().remote
    }

    pub fn remote_nonce_mut(&mut self) -> &mut Nonce {
        &mut self.nonce_pair.as_mut().unwrap().remote
    }

    fn init_reducer(tx_id: u64, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&tx_id).unwrap();
        let shared_state = state.shared();
        let graylist = shared_state.graylist.get(transaction).unwrap();

        if let TransactionType::TxHandshake(tx_state) = transaction.tx_type_mut() {
            if graylist.contains(&tx_state.address.ip()) {
                tx_state.result = Some(HandshakeResult::Graylisted);
                tx_state.stage.set_stage(HandshakeStage::Finish);
                drop(transactions);
                drop(shared_state);
                CompleteTransactionAction::new(tx_id).dispatch_pure(state);
            } else {
                drop(transactions);
                drop(shared_state);
                HandshakeSendConnectionMessageAction::new(tx_id).dispatch_pure(state);
            }
        } else {
            panic!("TxHandshake init_reducer: Invalid Transaction type")
        }
    }

    fn commit_reducer(tx_id: u64, state: &mut GlobalState) {
        let transactions = state.transactions();
        let transaction = transactions.get(&tx_id).unwrap();
        let mut shared_state = state.shared_mut();

        if let TransactionType::TxHandshake(tx_state) = transaction.tx_type() {
            let result = tx_state.result.clone().unwrap();

            if result == HandshakeResult::Success {
                let peer = Peer {
                    token: tx_state.token,
                    pub_key: tx_state.pub_key.clone().unwrap(),
                    nonce_pair: tx_state.nonce_pair.clone().unwrap(),
                    precomputed_key: tx_state.precomputed_key.clone().unwrap(),
                };

                let connected_peers = shared_state.connected_peers.get_mut(transaction).unwrap();
                connected_peers.insert(tx_state.address, peer);
            } else {
                let failed_peers = shared_state.failed_peers.get_mut(transaction).unwrap();
                failed_peers.insert(tx_state.token);

                if let HandshakeResult::Nack(_) = result {
                    return;
                }

                // Any errors during handshake result in graylisting the peer
                let graylist = shared_state.graylist.get_mut(transaction).unwrap();
                graylist.insert(tx_state.address.ip());
            }
        } else {
            panic!("TxHandshake commit_reducer: Invalid Transaction type")
        }
    }
}

impl Transaction for TxHandshake {
    fn init_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::init_reducer
    }

    fn commit_reducer_fn(&self) -> fn(u64, &mut GlobalState) {
        Self::commit_reducer
    }
}

pub struct HandshakeSendConnectionMessageAction {
    tx_id: u64,
}

impl HandshakeSendConnectionMessageAction {
    pub fn new(tx_id: u64) -> Self {
        Self { tx_id }
    }
}

impl PureAction for HandshakeSendConnectionMessageAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxHandshake(tx_state) => {
                let conf = &state.immutable().config;

                // TODO: proper error handling
                let msg = ConnectionMessage::try_new(
                    tx_state.address.port(),
                    &conf.identity.public_key,
                    &conf.identity.proof_of_work_stamp,
                    tx_state.nonce.clone(),
                    conf.network.clone(),
                )
                .unwrap();

                tx_state.local_chunk =
                    Some(BinaryChunk::from_content(&msg.as_bytes().unwrap()).unwrap());
                let send_connection_msg =
                    TxSendConnectionMessage::try_new(self.tx_id, tx_state.token, msg).unwrap();
                drop(transactions);
                CreateTransactionAction::new(&[], &[], send_connection_msg.into())
                    .dispatch_pure(state);
            }
            _ => panic!("HandshakeSendConnectionMessageAction reducer: Invalid Transaction type"),
        }
    }
}

pub struct HandshakeSendConnectionMessageCompleteAction {
    tx_id: u64,
    result: SendDataResult,
}

impl HandshakeSendConnectionMessageCompleteAction {
    pub fn new(tx_id: u64, result: SendDataResult) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for HandshakeSendConnectionMessageCompleteAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxHandshake(tx_state) => {
                if self.result == SendDataResult::Error {
                    tx_state.result = Some(HandshakeResult::SendConnectionError);
                    tx_state.stage.set_stage(HandshakeStage::Finish);
                    drop(transactions);
                    CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                } else {
                    let token = tx_state.token;
                    tx_state.stage.set_stage(HandshakeStage::RecvConnection);

                    drop(transactions);
                    CreateTransactionAction::new(
                        &[],
                        &[],
                        TxRecvConnectionMessage::new(self.tx_id, token).into(),
                    )
                    .dispatch_pure(state);
                }
            }
            _ => panic!(
                "HandshakeSendConnectionMessageCompleteAction reducer: Invalid Transaction type"
            ),
        }
    }
}

pub struct HandshakeRecvConnectionMessageAction {
    tx_id: u64,
    result: Result<ConnectionMessage, ()>,
}

impl HandshakeRecvConnectionMessageAction {
    pub fn new(tx_id: u64, result: Result<ConnectionMessage, ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for HandshakeRecvConnectionMessageAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();
        let shared_state = state.shared();
        // TODO: handle errors properly
        let connected_peers = shared_state.connected_peers.get(transaction).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxHandshake(tx_state) => {
                match &self.result {
                    Ok(msg) => {
                        let conf = &state.immutable().config;

                        //  TODO: check network version compatibility
                        if tx_state.check_pow(&msg, conf.pow_target).is_ok()
                            && tx_state.set_nonce_pair(&msg).is_ok()
                            && tx_state.set_pub_key(&msg).is_ok()
                        {
                            if *tx_state.pub_key.as_ref().unwrap()
                                == state.immutable().config.identity.public_key
                            {
                                tx_state.result = Some(HandshakeResult::ConnectSelfError);
                                tx_state.stage.set_stage(HandshakeStage::Finish);
                                drop(transactions);
                                drop(shared_state);
                                CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                            } else {
                                let precomputed_key = PrecomputedKey::precompute(
                                    tx_state.pub_key.as_ref().unwrap(),
                                    &conf.identity.secret_key,
                                );
                                tx_state.precomputed_key = Some(precomputed_key);

                                if connected_peers.len() == conf.max_peer_threshold {
                                    tx_state.nack_motive = Some(NackMotive::TooManyConnections)
                                }

                                if connected_peers.iter().any(|(_, peer)| {
                                    peer.pub_key == *tx_state.pub_key.as_ref().unwrap()
                                }) {
                                    tx_state.nack_motive = Some(NackMotive::AlreadyConnected)
                                }

                                tx_state.stage.set_stage(HandshakeStage::SendMetadata);
                                drop(transactions);
                                drop(shared_state);
                                HandshakeSendMetadataAction::new(self.tx_id).dispatch_pure(state)
                            }
                        }
                    }
                    _ => {
                        tx_state.result = Some(HandshakeResult::RecvConnectionError);
                        tx_state.stage.set_stage(HandshakeStage::Finish);
                        drop(transactions);
                        drop(shared_state);
                        CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                    }
                };
            }
            _ => panic!("HandshakeRecvConnectionMessageAction reducer: Invalid Transaction type"),
        }
    }
}

pub struct HandshakeSendMetadataAction {
    tx_id: u64,
}

impl HandshakeSendMetadataAction {
    pub fn new(tx_id: u64) -> Self {
        Self { tx_id }
    }
}

impl PureAction for HandshakeSendMetadataAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxHandshake(tx_state) => {
                let conf = &state.immutable().config;
                let msg = MetadataMessage::new(conf.disable_mempool, conf.private_node);
                // TODO: properly handle errors
                let tx = TxSendMetadataMessage::try_new(
                    self.tx_id,
                    tx_state.token,
                    tx_state.local_nonce().clone(),
                    tx_state.precomputed_key.as_ref().unwrap().clone(),
                    msg,
                )
                .unwrap();
                drop(transactions);
                CreateTransactionAction::new(&[], &[], tx.into()).dispatch_pure(state);
            }
            _ => panic!("HandshakeSendMetadataAction reducer: Invalid Transaction type"),
        }
    }
}

pub struct HandshakeSendMetadataMessageCompleteAction {
    tx_id: u64,
    nonce: Nonce,
    result: SendDataResult,
}

impl HandshakeSendMetadataMessageCompleteAction {
    pub fn new(tx_id: u64, nonce: Nonce, result: SendDataResult) -> Self {
        Self {
            tx_id,
            nonce,
            result,
        }
    }
}

impl PureAction for HandshakeSendMetadataMessageCompleteAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxHandshake(tx_state) => {
                *tx_state.local_nonce_mut() = self.nonce.clone();

                if self.result == SendDataResult::Error {
                    tx_state.result = Some(HandshakeResult::SendMetadataError);
                    tx_state.stage.set_stage(HandshakeStage::Finish);
                    drop(transactions);
                    CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                } else {
                    let tx = TxRecvMetadataMessage::new(
                        self.tx_id,
                        tx_state.token,
                        tx_state.remote_nonce().clone(),
                        tx_state.precomputed_key.as_ref().unwrap().clone(),
                    );

                    tx_state.stage.set_stage(HandshakeStage::RecvMetadata);
                    drop(transactions);
                    CreateTransactionAction::new(&[], &[], tx.into()).dispatch_pure(state);
                }
            }
            _ => panic!(
                "HandshakeSendMetadataMessageCompleteAction reducer: Invalid Transaction type"
            ),
        }
    }
}

pub struct HandshakeRecvMetadataMessageAction {
    tx_id: u64,
    nonce: Nonce,
    result: Result<MetadataMessage, ()>,
}

impl HandshakeRecvMetadataMessageAction {
    pub fn new(tx_id: u64, nonce: Nonce, result: Result<MetadataMessage, ()>) -> Self {
        Self {
            tx_id,
            nonce,
            result,
        }
    }
}

impl PureAction for HandshakeRecvMetadataMessageAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxHandshake(tx_state) => {
                *tx_state.remote_nonce_mut() = self.nonce.clone();

                match &self.result {
                    Ok(_metadata) => {
                        // TODO: check metadata information?
                        tx_state.stage.set_stage(HandshakeStage::SendAck);
                        drop(transactions);
                        HandshakeSendAckMessageAction::new(self.tx_id).dispatch_pure(state);
                    }
                    Err(_) => {
                        tx_state.result = Some(HandshakeResult::RecvMetadataError);
                        tx_state.stage.set_stage(HandshakeStage::Finish);
                        drop(transactions);
                        CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                    }
                }
            }
            _ => panic!("HandshakeRecvMetadataMessageAction reducer: Invalid Transaction type"),
        }
    }
}

pub struct HandshakeSendAckMessageAction {
    tx_id: u64,
}

impl HandshakeSendAckMessageAction {
    pub fn new(tx_id: u64) -> Self {
        Self { tx_id }
    }
}

impl PureAction for HandshakeSendAckMessageAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxHandshake(tx_state) => {
                let msg = match &tx_state.nack_motive {
                    Some(motive) => {
                        let potential_peers = []; // TODO
                        AckMessage::Nack(NackInfo::new(motive.clone(), &potential_peers))
                    }
                    None => AckMessage::Ack,
                };

                // TODO: properly handle error
                let tx = TxSendAckMessage::try_new(
                    self.tx_id,
                    tx_state.token,
                    tx_state.local_nonce().clone(),
                    tx_state.precomputed_key.as_ref().unwrap().clone(),
                    msg,
                )
                .unwrap();
                drop(transactions);
                CreateTransactionAction::new(&[], &[], tx.into()).dispatch_pure(state);
            }
            _ => panic!("HandshakeSendAckMessageAction reducer: Invalid Transaction type"),
        }
    }
}

pub struct HandshakeSendAckMessageCompleteAction {
    tx_id: u64,
    nonce: Nonce,
    result: SendDataResult,
}

impl HandshakeSendAckMessageCompleteAction {
    pub fn new(tx_id: u64, nonce: Nonce, result: SendDataResult) -> Self {
        Self {
            tx_id,
            nonce,
            result,
        }
    }
}

impl PureAction for HandshakeSendAckMessageCompleteAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxHandshake(tx_state) => {
                *tx_state.local_nonce_mut() = self.nonce.clone();

                if self.result == SendDataResult::Error {
                    tx_state.result = Some(HandshakeResult::SendAckError);
                    tx_state.stage.set_stage(HandshakeStage::Finish);
                    drop(transactions);
                    CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                } else {
                    // if we sent a Nack then we won't try to receive one
                    if tx_state.nack_motive.is_some() {
                        tx_state.result = Some(HandshakeResult::Nack(None));
                        tx_state.stage.set_stage(HandshakeStage::Finish);
                        drop(transactions);
                        CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
                    } else {
                        let tx = TxRecvAckMessage::new(
                            self.tx_id,
                            tx_state.token,
                            tx_state.remote_nonce().clone(),
                            tx_state.precomputed_key.as_ref().unwrap().clone(),
                        );

                        tx_state.stage.set_stage(HandshakeStage::RecvAck);
                        drop(transactions);
                        CreateTransactionAction::new(&[], &[], tx.into()).dispatch_pure(state);
                    }
                }
            }
            _ => panic!("HandshakeSendAckMessageCompleteAction reducer: Invalid Transaction type"),
        }
    }
}

pub struct HandshakeRecvAckMessageAction {
    tx_id: u64,
    nonce: Nonce,
    result: Result<AckMessage, ()>,
}

impl HandshakeRecvAckMessageAction {
    pub fn new(tx_id: u64, nonce: Nonce, result: Result<AckMessage, ()>) -> Self {
        Self {
            tx_id,
            nonce,
            result,
        }
    }
}

impl PureAction for HandshakeRecvAckMessageAction {
    fn reducer(&self, state: &mut GlobalState) {
        let mut transactions = state.transactions_mut();
        let transaction = transactions.get_mut(&self.tx_id).unwrap();

        match transaction.tx_type_mut() {
            TransactionType::TxHandshake(tx_state) => {
                *tx_state.remote_nonce_mut() = self.nonce.clone();

                let result = match self.result.clone() {
                    Ok(AckMessage::Ack) => HandshakeResult::Success,
                    Ok(AckMessage::NackV0) => HandshakeResult::Nack(None),
                    Ok(AckMessage::Nack(nack_info)) => HandshakeResult::Nack(Some(nack_info)),
                    Err(_) => HandshakeResult::RecvAckError,
                };

                tx_state.result = Some(result);
                tx_state.stage.set_stage(HandshakeStage::Finish);
                drop(transactions);
                CompleteTransactionAction::new(self.tx_id).dispatch_pure(state);
            }
            _ => panic!("HandshakeRecvAckMessageAction reducer: Invalid Transaction type"),
        }
    }
}
