use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use futures::io::AsyncWrite;
use futures::io::WriteHalf;
use romio::TcpStream;

/// Holds response channel for RPC messages.
pub struct RpcResponse(WriteHalf<TcpStream>);

impl RpcResponse {
    pub fn new(tx: WriteHalf<TcpStream>) -> RpcResponse {
        RpcResponse(tx)
    }
}

impl AsyncWrite for RpcResponse {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_close(cx)
    }
}