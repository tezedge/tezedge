use futures::io::AsyncWrite;
use futures::io::WriteHalf;
use futures::task::{Poll, Waker};
use romio::TcpStream;

/// Holds response channel for RPC messages.
pub struct RpcResponse(WriteHalf<TcpStream>);

impl RpcResponse {
    pub fn new(tx: WriteHalf<TcpStream>) -> RpcResponse {
        RpcResponse(tx)
    }
}

impl AsyncWrite for RpcResponse {
    fn poll_write(&mut self, waker: &Waker, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        self.0.poll_write(waker, buf)
    }

    fn poll_flush(&mut self, waker: &Waker) -> Poll<std::io::Result<()>> {
        self.0.poll_flush(waker)
    }

    fn poll_close(&mut self, waker: &Waker) -> Poll<std::io::Result<()>> {
        self.0.poll_close(waker)
    }
}
