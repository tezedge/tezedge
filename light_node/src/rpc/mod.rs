pub mod http;
pub mod message;
pub mod server;
pub mod response;

use futures::channel::mpsc;
use crate::rpc::message::RpcMessage;

pub use response::RpcResponse;

pub type RpcRx = mpsc::Receiver<(RpcMessage, Option<RpcResponse>)>;
pub type RpcTx = mpsc::Sender<(RpcMessage, Option<RpcResponse>)>;
