use futures::channel::mpsc;
use crate::tezos::p2p::message::P2pMessage;

pub type P2pRx = mpsc::Receiver<P2pMessage>;

#[macro_use]
pub mod message;
pub mod stream;
pub mod peer;
pub mod pool;
pub mod client;
pub mod node;

