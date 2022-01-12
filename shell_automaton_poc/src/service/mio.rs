use crypto::nonce::Nonce;
use either::Either;
use mio::net::{TcpListener, TcpSocket, TcpStream};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Write},
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use crate::{
    action::{ImpureAction, PureAction},
    state::{GlobalState, SharedStateField},
    transaction::{
        data_transfer::{RecvDataAction, RecvDataStage, SendDataAction, SendDataStage},
        handshake::handshake::TxHandshake,
        transaction::{
            CancelTransactionAction, CancelTransactionReason, CreateTransactionAction,
            TransactionType,
        },
    },
};

pub struct MioPeer {
    pub address: SocketAddr,
    pub stream: TcpStream,
    pub can_read: bool,
    pub can_write: bool,
}

impl MioPeer {
    pub fn new(address: SocketAddr, stream: TcpStream) -> Self {
        Self {
            address,
            stream,
            can_read: false,
            can_write: false,
        }
    }
}

struct Peers {
    peer_id: usize,
    peers: BTreeMap<mio::Token, MioPeer>,
}

impl Peers {
    pub fn new() -> Self {
        Self {
            peer_id: 0,
            peers: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, token: mio::Token, address: SocketAddr, stream: TcpStream) {
        self.peers
            .insert(token, MioPeer::new(address.into(), stream));
    }

    pub fn remove(&mut self, token: &mio::Token) -> MioPeer {
        // TODO: properly handle failure
        self.peers.remove(&token).unwrap()
    }

    pub fn get(&self, token: &mio::Token) -> &MioPeer {
        // TODO: properly handle error
        self.peers.get(token).unwrap()
    }

    pub fn get_mut(&mut self, token: &mio::Token) -> &mut MioPeer {
        // TODO: properly handle error
        self.peers.get_mut(token).unwrap()
    }

    pub fn ready(&self) -> BTreeSet<mio::Token> {
        self.peers
            .iter()
            .filter_map(|(token, peer)| {
                if peer.can_read || peer.can_write {
                    Some(*token)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn try_new_token(&mut self) -> Result<mio::Token, MioServiceError> {
        /*
            We need all this logic to re-use IDs because `mio::Token` uses `usize` internally,
            which means it is possible to reach the maximum value in 32-bit architectures.
        */
        let mut iterations: usize = 0;

        while iterations < usize::MAX {
            let token = mio::Token(self.peer_id);

            self.peer_id += 1;

            // don't use `MIO_SERVER_TOKEN`
            if self.peer_id == usize::MAX {
                self.peer_id = 0;
            }

            if self.peers.contains_key(&token) == false {
                return Ok(token);
            }

            iterations += 1;
        }

        Err(MioServiceError::TokensExhausted)
    }
}

pub struct MioService {
    poll: mio::Poll,
    poll_timeout: Duration,
    events: mio::Events,
    server: TcpListener,
    peers: Peers,
}

impl MioService {
    const DEFAULT_BACKLOG_SIZE: u32 = 255;
    const MIO_SERVER_TOKEN: mio::Token = mio::Token(usize::MAX);

    pub fn try_new(listen_addr: SocketAddr) -> Result<Self, MioServiceError> {
        let socket = match listen_addr.ip() {
            IpAddr::V4(_) => TcpSocket::new_v4(),
            IpAddr::V6(_) => TcpSocket::new_v6(),
        };

        // TODO: handle error properly
        let poll = mio::Poll::new().unwrap();

        match socket {
            Ok(socket) => {
                if socket.set_reuseaddr(true).is_ok() && socket.bind(listen_addr).is_ok() {
                    match socket.listen(Self::DEFAULT_BACKLOG_SIZE) {
                        Ok(mut server) => {
                            if poll
                                .registry()
                                .register(
                                    &mut server,
                                    Self::MIO_SERVER_TOKEN,
                                    mio::Interest::READABLE,
                                )
                                .is_ok()
                            {
                                Ok(Self {
                                    poll,
                                    poll_timeout: Duration::from_millis(100),
                                    events: mio::Events::with_capacity(1024),
                                    server,
                                    peers: Peers::new(),
                                })
                            } else {
                                Err(MioServiceError::PollRegisterError)
                            }
                        }
                        _ => Err(MioServiceError::ListenError),
                    }
                } else {
                    Err(MioServiceError::BindError)
                }
            }
            _ => Err(MioServiceError::SocketError),
        }
    }

    fn try_register_new(
        &mut self,
        mut stream: TcpStream,
        address: SocketAddr,
    ) -> Result<(mio::Token, SocketAddr), MioServiceError> {
        let token = self.peers.try_new_token()?;

        match self.poll.registry().register(
            &mut stream,
            token,
            mio::Interest::READABLE | mio::Interest::WRITABLE,
        ) {
            Ok(_) => {
                self.peers.add(token, address, stream);
                Ok((token, address))
            }
            _ => Err(MioServiceError::RegisterError((token, address))),
        }
    }

    fn accept(&mut self) -> Result<(mio::Token, SocketAddr), MioServiceError> {
        match self.server.accept() {
            Ok((stream, address)) => self.try_register_new(stream, address),
            Err(err) => {
                if let io::ErrorKind::WouldBlock = err.kind() {
                    Err(MioServiceError::AcceptWouldBlock)
                } else {
                    Err(MioServiceError::AcceptError)
                }
            }
        }
    }

    pub fn send(&mut self, token: &mio::Token, data: &Vec<u8>) -> Result<usize, ()> {
        let peer = self.peers.get_mut(token);

        match peer.stream.write(data.as_slice()) {
            Ok(bytes_written) => Ok(bytes_written),
            Err(err) => {
                match err.kind() {
                    io::ErrorKind::WouldBlock => {
                        panic!("send called w/o can_write");
                        //Ok(0)
                    }
                    _ => Err(()),
                }
            }
        }
    }

    pub fn recv(&mut self, token: &mio::Token, len: usize) -> Result<Vec<u8>, ()> {
        let peer = self.peers.get_mut(token);
        let mut buffer: Vec<u8> = vec![0; len];

        match peer.stream.read(&mut buffer[0..len]) {
            Ok(bytes_read) => {
                buffer.resize(bytes_read, 0);
                Ok(buffer)
            }
            Err(err) => {
                match err.kind() {
                    io::ErrorKind::WouldBlock => {
                        panic!("recv called w/o can_read");
                        //buffer.resize(0, 0);
                        //Ok(buffer)
                    }
                    _ => Err(()),
                }
            }
        }
    }

    pub fn disconnect(&mut self, token: mio::Token) {
        // TODO: handle errors
        self.poll
            .registry()
            .deregister(&mut self.peers.remove(&token).stream)
            .unwrap();
    }

    pub fn make_progress(&mut self, state: &mut GlobalState) {
        if let Err(err) = self.poll.poll(&mut self.events, Some(self.poll_timeout)) {
            panic!("Mio poll error: {:?}", err);
        }

        let events: Vec<(mio::Token, bool, bool, bool)> = self
            .events
            .iter()
            .map(|event| {
                (
                    event.token(),
                    event.is_readable(),
                    event.is_writable(),
                    event.is_error() || event.is_read_closed() || event.is_write_closed(),
                )
            })
            .collect();

        for (token, can_read, can_write, is_error_or_closed) in events.iter() {
            match *token {
                // incoming connection(s) event
                Self::MIO_SERVER_TOKEN => {
                    loop {
                        match self.accept() {
                            Ok((token, address)) => {
                                /*
                                    Create a new `TxHandshake` transaction, these transactions are mutually
                                    exclusive, so if there are multiple connections the first transaction will
                                    get into `Pending` state while following ones will be set to `Retry`.

                                    When the first transaction finishes (a connection is fully established or
                                    failure/timeout happens) the next `Retry` transaction is moved into `Pending`
                                    state and so on.

                                    A limitation of mutual exclusive handshakes is that if a peer is too slow
                                    (or times out) it will delay any other pending connections. On the other hand
                                    this approach grants atomicity to shared-state's `ConnectedPeers` and
                                    `Graylist` fields, thus avoiding race conditions by concurrent connections.
                                    Also, if a transaction is cancelled we can make sure that no changes are made
                                    to the shared-state.
                                */
                                let nonce = Nonce::random(); // TODO: implement "random" service?

                                CreateTransactionAction::new(
                                    &[SharedStateField::ConnectedPeers, SharedStateField::Graylist],
                                    &[SharedStateField::ConnectedPeers],
                                    TxHandshake::new(token, address, true, nonce).into(),
                                )
                                .dispatch_pure(state);
                            }
                            Err(err) => {
                                println!("accept error {:?}", err);
                                break;
                            }
                        }
                    }
                }
                token => {
                    let peer = self.peers.get_mut(&token);

                    peer.can_read = *can_read;
                    peer.can_write = *can_write;

                    if *is_error_or_closed {
                        MioEventConnectionClosedAction::new(token).dispatch_impure(state, self);
                    }
                }
            }
        }

        /*
            Call effects (by dispatching impure actions) for every peer that is enabled to perform read
            and/or write operations.

            This is done in a level-triggered fashion: the actions' effects handler must set `can_read`
            and/or `can_write` to `false` to acknowledge the event(s).
        */
        let peers_set = self.peers.ready();
        let transactions = state.transactions();
        let mio_actions: Vec<Either<MioCanReadAction, MioCanWriteAction>> = transactions
            .iter()
            .filter(|(_, tx)| tx.is_pending())
            .filter_map(|(tx_id, tx)| match tx.tx_type() {
                TransactionType::TxRecvData(tx_state)
                    if peers_set.contains(tx_state.token())
                        && self.peers.get(tx_state.token()).can_read =>
                {
                    Some(Either::Left(MioCanReadAction::new(*tx_id)))
                }
                TransactionType::TxSendData(tx_state)
                    if peers_set.contains(tx_state.token())
                        && self.peers.get(tx_state.token()).can_write =>
                {
                    Some(Either::Right(MioCanWriteAction::new(*tx_id)))
                }
                _ => None,
            })
            .collect();

        drop(transactions);
        mio_actions.iter().for_each(|action| match action {
            Either::Left(read_action) => read_action.dispatch_impure(state, self),
            Either::Right(write_action) => write_action.dispatch_impure(state, self),
        })
    }
}

#[derive(Debug)]
pub enum MioServiceError {
    PollRegisterError,
    ListenError,
    BindError,
    SocketError,
    AcceptWouldBlock,
    AcceptError,
    TokensExhausted,
    RegisterError((mio::Token, SocketAddr)),
}

pub struct MioEventConnectionClosedAction {
    token: mio::Token,
}

impl MioEventConnectionClosedAction {
    pub fn new(token: mio::Token) -> Self {
        Self { token }
    }
}

impl ImpureAction<MioService> for MioEventConnectionClosedAction {
    fn reducer(&self, state: &mut GlobalState) {
        /*
            Find transactions affected by disconnection event. Even if the transaction is in "Retry"
            state we will cancel it to free resources ASAP.
        */
        let handshake_tx = state
            .transactions()
            .iter()
            .find_map(|(tx_id, tx)| match tx.tx_type() {
                TransactionType::TxHandshake(tx_state) if *tx_state.token() == self.token => {
                    Some(*tx_id)
                }
                _ => None,
            });

        if let Some(tx_id) = handshake_tx {
            drop(handshake_tx);
            CancelTransactionAction::new(tx_id, CancelTransactionReason::ConnectionClosed)
                .dispatch_pure(state);
        } else {
            panic!("MioEventConnectionClosedAction reducer: transaction not found");
        }
    }

    fn effects(&self, _state: &mut GlobalState, service: &mut MioService) {
        service.disconnect(self.token);
    }
}

pub struct MioCanReadAction {
    tx_id: u64,
}

impl MioCanReadAction {
    pub fn new(tx_id: u64) -> Self {
        Self { tx_id }
    }
}

impl ImpureAction<MioService> for MioCanReadAction {
    fn reducer(&self, _state: &mut GlobalState) {}

    fn effects(&self, state: &mut GlobalState, service: &mut MioService) {
        let transactions = state.transactions();
        let transaction = transactions.get(&self.tx_id).unwrap();

        if let TransactionType::TxRecvData(tx_state) = transaction.tx_type() {
            match tx_state.stage() {
                RecvDataStage::Receiving => {
                    let token = tx_state.token();
                    let result = service.recv(token, tx_state.bytes_remaining());
                    let peer = service.peers.get_mut(token);

                    peer.can_read = false;
                    drop(transactions);
                    RecvDataAction::new(self.tx_id, result).dispatch_pure(state);
                }
                RecvDataStage::Finish => panic!("MioCanReadAction effects called at invalid stage"),
            }
        } else {
            panic!("MioCanReadAction effects called with invalid transaction typre")
        }
    }
}
pub struct MioCanWriteAction {
    tx_id: u64,
}

impl MioCanWriteAction {
    pub fn new(tx_id: u64) -> Self {
        Self { tx_id }
    }
}

impl ImpureAction<MioService> for MioCanWriteAction {
    fn reducer(&self, _state: &mut GlobalState) {}

    fn effects(&self, state: &mut GlobalState, service: &mut MioService) {
        let transactions = state.transactions();
        let transaction = transactions.get(&self.tx_id).unwrap();

        if let TransactionType::TxSendData(tx_state) = transaction.tx_type() {
            match tx_state.stage() {
                SendDataStage::Transmitting => {
                    let token = tx_state.token();
                    let result = service.send(token, tx_state.bytes_to_send());
                    let peer = service.peers.get_mut(token);

                    peer.can_write = false;
                    drop(transactions);
                    SendDataAction::new(self.tx_id, result).dispatch_pure(state);
                }
                SendDataStage::Finish => {
                    panic!("MioCanWriteAction effects called at invalid stage")
                }
            }
        } else {
            panic!("MioCanWriteAction effects called with invalid transaction typre")
        }
    }
}
