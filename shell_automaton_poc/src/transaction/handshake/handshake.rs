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
    transaction::transaction::{
        CompleteTransactionAction, CreateTransactionAction, ParentInfo, Transaction,
        TransactionStage, TransactionState, TransactionType, TxOps,
    },
};

use super::{
    ack::{TxRecvAckMessage, TxSendAckMessage},
    connection::{TxRecvConnectionMessage, TxSendConnectionMessage},
    metadata::{TxRecvMetadataMessage, TxSendMetadataMessage},
};

#[derive(PartialEq, Clone)]
pub enum HandshakeError {
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
    TooManyConnections,
    AlreadyConnected,
    RecvAckError,
    RecvNack,
}
#[derive(Clone)]
pub struct InitContext {
    token: mio::Token,
    address: SocketAddr,
    incoming: bool,
    nonce: Nonce,
}
#[derive(Clone)]
pub struct SendConnectionContext {
    token: mio::Token,
    address: SocketAddr,
    incoming: bool,
    local_chunk: BinaryChunk,
}
#[derive(Clone)]
pub struct RecvConnectionContext {
    token: mio::Token,
    address: SocketAddr,
    incoming: bool,
    local_chunk: BinaryChunk,
}
#[derive(Clone)]
pub struct SendMetadataContext {
    token: mio::Token,
    address: SocketAddr,
    nonce_pair: NoncePair,
    precomputed_key: PrecomputedKey,
}
#[derive(Clone)]
pub struct RecvMetadataContext {
    token: mio::Token,
    address: SocketAddr,
    nonce_pair: NoncePair,
    precomputed_key: PrecomputedKey,
}
#[derive(Clone)]
pub struct SendAckContext {
    token: mio::Token,
    address: SocketAddr,
    nonce_pair: NoncePair,
    precomputed_key: PrecomputedKey,
    msg: AckMessage,
}
#[derive(Clone)]
pub struct RecvAckContext {
    token: mio::Token,
    address: SocketAddr,
    nonce_pair: NoncePair,
    precomputed_key: PrecomputedKey,
}
#[derive(Clone)]
pub struct CompletedContext {
    token: mio::Token,
    address: SocketAddr,
    nonce_pair: NoncePair,
    precomputed_key: PrecomputedKey,
}
#[derive(Clone)]
pub struct ErrorContext {
    token: mio::Token,
    address: SocketAddr,
    reason: HandshakeError,
}

#[derive(Clone)]
pub enum TxHandshake {
    Init(InitContext),
    SendConnection(SendConnectionContext),
    RecvConnection(RecvConnectionContext),
    SendMetadata(SendMetadataContext),
    RecvMetadata(RecvMetadataContext),
    SendAck(SendAckContext),
    RecvAck(RecvAckContext),
    Completed(Result<CompletedContext, ErrorContext>),
}

impl TransactionStage for TxHandshake {
    fn is_enabled(&self, next_stage: &Self) -> bool {
        match self {
            Self::Init(_) => match next_stage {
                Self::Completed(Err(_)) => true,
                Self::SendConnection(_) => true,
                _ => false,
            },
            Self::SendConnection(_) => match next_stage {
                Self::Completed(Err(_)) => true,
                Self::RecvConnection(_) => true,
                _ => false,
            },
            Self::RecvConnection(_) => match next_stage {
                Self::Completed(Err(_)) => true,
                Self::SendMetadata(_) => true,
                _ => false,
            },
            Self::SendMetadata(_) => match next_stage {
                Self::Completed(Err(_)) => true,
                Self::RecvMetadata(_) => true,
                _ => false,
            },
            Self::RecvMetadata(_) => match next_stage {
                Self::Completed(Err(_)) => true,
                Self::SendAck(_) => true,
                _ => false,
            },
            Self::SendAck(_) => match next_stage {
                Self::Completed(Err(_)) => true,
                Self::RecvAck(_) => true,
                _ => false,
            },
            Self::RecvAck(_) => match next_stage {
                Self::Completed(_) => true,
                _ => false,
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

impl Transaction for TxHandshake {
    fn init(&mut self, tx_id: u64, state: &GlobalState) {
        assert!(self.is_init());

        if let TxHandshake::Init(ctx) = self.clone() {
            if state
                .shared
                .peers
                .borrow(&state.access(tx_id))
                .iter()
                .any(|peer| {
                    if let Peer::Graylisted(address) = peer {
                        *address == ctx.address.ip()
                    } else {
                        false
                    }
                })
            {
                self.set(Self::Completed(Err(ErrorContext {
                    token: ctx.token,
                    address: ctx.address,
                    reason: HandshakeError::Graylisted,
                })));
                CompleteTransactionAction::<TxHandshake>::new(tx_id).dispatch_pure(state);
                return;
            }

            let conf = &state.immutable.config;
            let msg = ConnectionMessage::try_new(
                ctx.address.port(),
                &conf.identity.public_key,
                &conf.identity.proof_of_work_stamp,
                ctx.nonce.clone(),
                conf.network.clone(),
            )
            .unwrap();

            self.set(Self::SendConnection(SendConnectionContext {
                token: ctx.token,
                address: ctx.address,
                incoming: ctx.incoming,
                local_chunk: BinaryChunk::from_content(&msg.as_bytes().unwrap()).unwrap(),
            }));

            CreateTransactionAction::new(
                0,
                0,
                TxSendConnectionMessage::try_new(ctx.token, msg).unwrap(),
                Some(ParentInfo {
                    tx_id,
                    tx_type: TransactionType::TxHandshake,
                }),
            )
            .dispatch_pure(state);
        }
    }

    fn commit(&self, tx_id: u64, _parent: Option<ParentInfo>, state: &GlobalState) {
        assert!(self.is_completed());

        if let TxHandshake::Completed(result) = &self {
            let mut peers = state.shared.peers.borrow_mut(&state.access(tx_id));

            match result.clone() {
                Ok(ctx) => {
                    peers.insert(Peer::Connected(
                        ctx.token,
                        ctx.address,
                        ctx.precomputed_key,
                        ctx.nonce_pair,
                    ));
                }
                Err(ErrorContext {
                    address, reason, ..
                }) => {
                    if reason != HandshakeError::RecvNack
                        && reason != HandshakeError::Graylisted
                        && reason != HandshakeError::TooManyConnections
                    {
                        /*
                            Before graylisting a peer we need to see if there are any other peers with same IP
                            and remove them (this is possible because we support NAT). A better solution would
                            be to base graylisting not on IP addresses but on Peer-ID (from peer's identity).
                        */
                        let connected_peers_with_same_ip: Vec<Peer> = peers
                            .iter()
                            .filter_map(|peer| match peer {
                                Peer::Connected(_, sock_address, ..)
                                    if sock_address.ip() == address.ip() =>
                                {
                                    Some(peer.clone())
                                }
                                _ => None,
                            })
                            .collect();

                        for peer in connected_peers_with_same_ip {
                            if peers.remove(&peer) == false {
                                panic!("peer not in set")
                            }
                        }

                        peers.insert(Peer::Graylisted(address.ip()));
                    }
                }
            }
        }
    }

    fn cancel(&self, _tx_id: u64, _parent: Option<ParentInfo>, _state: &GlobalState) {}
}

impl TxHandshake {
    pub fn new(token: mio::Token, address: SocketAddr, incoming: bool, nonce: Nonce) -> Self {
        TxHandshake::Init(InitContext {
            token,
            address,
            incoming,
            nonce,
        })
    }

    pub fn token(&self) -> mio::Token {
        match self {
            Self::Init(InitContext { token, .. })
            | Self::SendConnection(SendConnectionContext { token, .. })
            | Self::RecvConnection(RecvConnectionContext { token, .. })
            | Self::SendMetadata(SendMetadataContext { token, .. })
            | Self::RecvMetadata(RecvMetadataContext { token, .. })
            | Self::SendAck(SendAckContext { token, .. })
            | Self::RecvAck(RecvAckContext { token, .. })
            | Self::Completed(Ok(CompletedContext { token, .. }))
            | Self::Completed(Err(ErrorContext { token, .. })) => *token,
        }
    }

    fn is_init(&self) -> bool {
        match self {
            Self::Init(_) => true,
            _ => false,
        }
    }
}

pub struct HandshakeSendConnectionMessageCompleteAction {
    tx_id: u64,
    result: Result<(), ()>,
}

impl HandshakeSendConnectionMessageCompleteAction {
    pub fn new(tx_id: u64, result: Result<(), ()>) -> Self {
        Self { tx_id, result }
    }
}

impl PureAction for HandshakeSendConnectionMessageCompleteAction {
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = TxHandshake::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            match tx_state.clone() {
                TxHandshake::SendConnection(ctx) => {
                    if self.result.is_err() {
                        tx_state.set(TxHandshake::Completed(Err(ErrorContext{token: ctx.token, address: ctx.address, reason: HandshakeError::SendConnectionError})));
                        //drop(transaction);
                        //drop(transactions);
                        CompleteTransactionAction::<TxHandshake>::new(self.tx_id)
                        .dispatch_pure(state);
                        return
                    }

                    tx_state.set(TxHandshake::RecvConnection(RecvConnectionContext {
                        token: ctx.token,
                        address: ctx.address,
                        incoming: ctx.incoming,
                        local_chunk: ctx.local_chunk
                    }));
                    //drop(transaction);
                    //drop(transactions);
                    CreateTransactionAction::new(
                        0,
                        0,
                        TxRecvConnectionMessage::new(ctx.token),
                        Some(ParentInfo {
                            tx_id: self.tx_id,
                            tx_type: TransactionType::TxHandshake,
                        }),
                    )
                    .dispatch_pure(state);
                },
                _ => panic!("HandshakeSendConnectionMessageCompleteAction reducer: action dispatched at invalid stage"),
            }
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
    fn reducer(&self, state: &GlobalState) {
        let conf = &state.immutable.config;
        let transactions = state.transactions.borrow();
        let mut transaction = TxHandshake::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            match tx_state.clone() {
                TxHandshake::RecvConnection(ctx) => {
                    match &self.result {
                        Ok(msg) => {
                            let pow_data = [msg.public_key.clone(), msg.proof_of_work_stamp.clone()].concat();

                            if crypto::proof_of_work::check_proof_of_work(pow_data.as_slice(), conf.pow_target).is_err() {
                                tx_state.set(TxHandshake::Completed(Err(ErrorContext{token: ctx.token, address: ctx.address, reason: HandshakeError::BadPow})));
                                //drop(transaction);
                                //drop(transactions);
                                CompleteTransactionAction::<TxHandshake>::new(self.tx_id)
                                .dispatch_pure(state);
                                return
                            }

                            let nonce_pair = generate_nonces(
                                ctx.local_chunk.raw(),
                                BinaryChunk::from_content(&msg.as_bytes().unwrap())
                                    .unwrap()
                                    .raw(),
                                ctx.incoming,
                            );

                            if nonce_pair.is_err() {
                                tx_state.set(TxHandshake::Completed(Err(ErrorContext{token: ctx.token, address: ctx.address, reason: HandshakeError::NoncePairError})));
                                //drop(transaction);
                                //drop(transactions);
                                CompleteTransactionAction::<TxHandshake>::new(self.tx_id)
                                .dispatch_pure(state);
                                return
                            }

                            let pub_key = PublicKey::from_bytes(&msg.public_key);

                            if pub_key.is_err() {
                                tx_state.set(TxHandshake::Completed(Err(ErrorContext{token: ctx.token, address: ctx.address, reason: HandshakeError::PublicKeyError})));
                                //drop(transaction);
                                //drop(transactions);
                                CompleteTransactionAction::<TxHandshake>::new(self.tx_id)
                                .dispatch_pure(state);
                                return
                            }

                            let pub_key = pub_key.unwrap();
                            let precomputed_key = PrecomputedKey::precompute(
                                &pub_key,
                                &conf.identity.secret_key,
                            );

                            if pub_key == conf.identity.public_key {
                                tx_state.set(TxHandshake::Completed(Err(ErrorContext{token: ctx.token, address: ctx.address, reason: HandshakeError::ConnectSelfError})));
                                //drop(transaction);
                                //drop(transactions);
                                CompleteTransactionAction::<TxHandshake>::new(self.tx_id)
                                .dispatch_pure(state);
                                return
                            }

                            let nonce_pair = nonce_pair.unwrap();
                            let tx = TxSendMetadataMessage::try_new(
                                ctx.token,
                                nonce_pair.local.clone(),
                                precomputed_key.clone(),
                                MetadataMessage::new(conf.disable_mempool, conf.private_node)
                            )
                            .unwrap();

                            tx_state.set(TxHandshake::SendMetadata(SendMetadataContext{
                                token: ctx.token,
                                address: ctx.address,
                                nonce_pair,
                                precomputed_key
                            }));

                            //drop(transaction);
                            //drop(transactions);
                            CreateTransactionAction::new(0, 0, tx, Some(ParentInfo {
                                tx_id: self.tx_id,
                                tx_type: TransactionType::TxHandshake,
                            })).dispatch_pure(state);
                        },
                        Err(_) => {
                            tx_state.set(TxHandshake::Completed(Err(ErrorContext{token: ctx.token, address: ctx.address, reason: HandshakeError::RecvConnectionError})));
                            //drop(transaction);
                            //drop(transactions);
                            CompleteTransactionAction::<TxHandshake>::new(self.tx_id)
                            .dispatch_pure(state);
                        }
                    }
                }
                _ => panic!("HandshakeRecvConnectionMessageAction reducer: action dispatched at invalid stage"),
            }
        }
    }
}

pub struct HandshakeSendMetadataMessageCompleteAction {
    tx_id: u64,
    nonce: Nonce,
    result: Result<(), ()>,
}

impl HandshakeSendMetadataMessageCompleteAction {
    pub fn new(tx_id: u64, nonce: Nonce, result: Result<(), ()>) -> Self {
        Self {
            tx_id,
            nonce,
            result,
        }
    }
}

impl PureAction for HandshakeSendMetadataMessageCompleteAction {
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = TxHandshake::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            match tx_state.clone() {
                TxHandshake::SendMetadata(ctx) => {
                    if self.result.is_err() {
                        tx_state.set(TxHandshake::Completed(Err(ErrorContext{token: ctx.token, address: ctx.address, reason: HandshakeError::SendMetadataError})));
                        //drop(transaction);
                        //drop(transactions);
                        CompleteTransactionAction::<TxHandshake>::new(self.tx_id)
                        .dispatch_pure(state);
                        return
                    }
                    let remote_nonce = ctx.nonce_pair.remote;
                    let tx = TxRecvMetadataMessage::new(ctx.token, remote_nonce.clone(), ctx.precomputed_key.clone());

                    tx_state.set(TxHandshake::RecvMetadata(RecvMetadataContext {
                        token: ctx.token,
                        address: ctx.address,
                        nonce_pair: NoncePair { local: self.nonce.clone(), remote: remote_nonce },
                        precomputed_key: ctx.precomputed_key
                    }));

                    //drop(transaction);
                    //drop(transactions);
                    CreateTransactionAction::new(
                        0,
                        0,
                        tx,
                        Some(ParentInfo {
                            tx_id: self.tx_id,
                            tx_type: TransactionType::TxHandshake,
                        }),
                    )
                    .dispatch_pure(state);
                },
                _ => panic!("HandshakeSendMetadataMessageCompleteAction reducer: action dispatched at invalid stage"),
            }
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
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let conf = &state.immutable.config;
        let mut transaction = TxHandshake::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            match tx_state.clone() {
                TxHandshake::RecvMetadata(ctx) => {
                    if self.result.is_err() {
                        tx_state.set(TxHandshake::Completed(Err(ErrorContext{token: ctx.token, address: ctx.address, reason: HandshakeError::RecvMetadataError})));
                        //drop(transaction);
                        //drop(transactions);
                        CompleteTransactionAction::<TxHandshake>::new(self.tx_id)
                        .dispatch_pure(state);
                        return
                    }
                    // TODO: check metadata information?

                    let nack = (||{
                        let mut connected_count = 0;
                        let peers = state.shared.peers.borrow(&state.access(self.tx_id));

                        for peer in peers.iter() {
                            if let Peer::Connected(_, _, precomputed_key, _) = peer {
                                /*
                                    NOTE: the following check makes the assumption that the precomputed key resulting
                                    from a peer's public key and ours is going to be always the same. A better approach
                                    would be to used the public-key instead.
                                */
                                if *precomputed_key == ctx.precomputed_key {
                                    return Some(NackMotive::AlreadyConnected)
                                }

                                connected_count += 1;
                            }
                        }

                        if connected_count == conf.max_peer_threshold {
                            Some(NackMotive::TooManyConnections)
                        } else {
                            None
                        }
                    })();

                    let msg = if let Some(nack_motive) = nack {
                        let potential_peers = []; // TODO
                        AckMessage::Nack(NackInfo::new(nack_motive, &potential_peers))
                    } else {
                        AckMessage::Ack
                    };

                    // TODO: properly handle error
                    let tx = TxSendAckMessage::try_new(
                        ctx.token,
                        ctx.nonce_pair.local.clone(),
                        ctx.precomputed_key.clone(),
                        msg.clone(),
                    )
                    .unwrap();

                    tx_state.set(TxHandshake::SendAck(SendAckContext {
                        token: ctx.token,
                        address: ctx.address,
                        nonce_pair: NoncePair { local: ctx.nonce_pair.local, remote: self.nonce.clone() },
                        precomputed_key: ctx.precomputed_key,
                        msg
                    }));

                    //drop(transaction);
                    //drop(transactions);
                    CreateTransactionAction::new(0, 0, tx, Some(ParentInfo {
                        tx_id: self.tx_id,
                        tx_type: TransactionType::TxHandshake,
                    })).dispatch_pure(state);
                },
                _ => panic!("HandshakeRecvMetadataMessageAction reducer: action dispatched at invalid stage"),
            }
        }
    }
}

pub struct HandshakeSendAckMessageCompleteAction {
    tx_id: u64,
    nonce: Nonce,
    result: Result<(), ()>,
}

impl HandshakeSendAckMessageCompleteAction {
    pub fn new(tx_id: u64, nonce: Nonce, result: Result<(), ()>) -> Self {
        Self {
            tx_id,
            nonce,
            result,
        }
    }
}

impl PureAction for HandshakeSendAckMessageCompleteAction {
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = TxHandshake::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            match tx_state.clone() {
                TxHandshake::SendAck(ctx) => {
                    if self.result.is_err() {
                        tx_state.set(TxHandshake::Completed(Err(ErrorContext{token: ctx.token, address: ctx.address, reason: HandshakeError::SendAckError})));
                        //drop(transaction);
                        //drop(transactions);
                        CompleteTransactionAction::<TxHandshake>::new(self.tx_id)
                        .dispatch_pure(state);
                        return
                    }

                    if let AckMessage::Nack(nack_info) = ctx.msg {
                        let reason = match nack_info.motive() {
                            NackMotive::TooManyConnections => HandshakeError::TooManyConnections,
                            NackMotive::AlreadyConnected => HandshakeError::AlreadyConnected,
                            _ => unreachable!()
                        };
                        tx_state.set(TxHandshake::Completed(Err(ErrorContext{token: ctx.token, address: ctx.address, reason})));
                        //drop(transaction);
                        //drop(transactions);
                        CompleteTransactionAction::<TxHandshake>::new(self.tx_id)
                        .dispatch_pure(state);
                        return
                    }

                    let tx = TxRecvAckMessage::new(
                        ctx.token,
                        ctx.nonce_pair.remote.clone(),
                        ctx.precomputed_key.clone(),
                    );

                    tx_state.set(TxHandshake::RecvAck(RecvAckContext {
                        token: ctx.token,
                        address: ctx.address,
                        nonce_pair: NoncePair { local: self.nonce.clone(), remote: ctx.nonce_pair.remote },
                        precomputed_key: ctx.precomputed_key,
                    }));

                    //drop(transaction);
                    //drop(transactions);
                    CreateTransactionAction::new(0, 0, tx, Some(ParentInfo {
                        tx_id: self.tx_id,
                        tx_type: TransactionType::TxHandshake,
                    })).dispatch_pure(state);
                },
                _ => panic!("HandshakeSendAckMessageCompleteAction reducer: action dispatched at invalid stage"),
            }
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
    fn reducer(&self, state: &GlobalState) {
        let transactions = state.transactions.borrow();
        let mut transaction = TxHandshake::get(&transactions, self.tx_id).borrow_mut();
        assert!(transaction.is_pending());

        if let TransactionState::Pending(tx_state) = &mut transaction.state {
            match tx_state.clone() {
                TxHandshake::RecvAck(ctx) => match self.result {
                    Ok(AckMessage::Ack) => {
                        tx_state.set(TxHandshake::Completed(Ok(CompletedContext {
                            token: ctx.token,
                            address: ctx.address,
                            nonce_pair: NoncePair {
                                local: ctx.nonce_pair.local,
                                remote: self.nonce.clone(),
                            },
                            precomputed_key: ctx.precomputed_key,
                        })))
                    }
                    Ok(AckMessage::NackV0) | Ok(AckMessage::Nack(_)) => {
                        tx_state.set(TxHandshake::Completed(Err(ErrorContext {
                            token: ctx.token,
                            address: ctx.address,
                            reason: HandshakeError::RecvNack,
                        })))
                    }
                    Err(_) => tx_state.set(TxHandshake::Completed(Err(ErrorContext {
                        token: ctx.token,
                        address: ctx.address,
                        reason: HandshakeError::RecvAckError,
                    }))),
                },
                _ => panic!(
                    "HandshakeRecvAckMessageAction reducer: action dispatched at invalid stage"
                ),
            }

            //drop(transaction);
            //drop(transactions);
            CompleteTransactionAction::<TxHandshake>::new(self.tx_id).dispatch_pure(state);
        }
    }
}
