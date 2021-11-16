// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::{mpsc, Arc};

/// Error when sending request to the responder/worker.
///
/// Can only happen if worker is disconnected/shut down.
///
/// Use [RequestSendError::payload] to retrieve request that failed to send.
#[derive(Debug)]
pub struct RequestSendError<T>(T);

impl<T> RequestSendError<T> {
    /// Retrieve request that failed to be sent to the worker.
    pub fn payload(self) -> T {
        self.0
    }
}

impl<T> From<mpsc::SendError<T>> for RequestSendError<T> {
    fn from(error: mpsc::SendError<T>) -> Self {
        Self(error.0)
    }
}

/// Error when sending response to the requester.
///
/// Can only happen if requester is disconnected/shut down.
///
/// Use [ResponseSendError::payload] to retrieve request that failed to send.
#[derive(Debug)]
pub struct ResponseSendError<T>(T);

impl<T> ResponseSendError<T> {
    /// Retrieve response that failed to be sent to the requester.
    pub fn payload(self) -> T {
        self.0
    }
}

impl<T> From<mpsc::SendError<T>> for ResponseSendError<T> {
    fn from(error: mpsc::SendError<T>) -> Self {
        Self(error.0)
    }
}

#[derive(Debug)]
pub enum ResponseTryRecvError {
    /// There is nothing to receive yet.
    Empty,

    /// Worker got disconnected/shut down.
    Disconnected,
}

impl From<mpsc::TryRecvError> for ResponseTryRecvError {
    fn from(error: mpsc::TryRecvError) -> Self {
        match error {
            mpsc::TryRecvError::Empty => Self::Empty,
            mpsc::TryRecvError::Disconnected => Self::Disconnected,
        }
    }
}

/// Error while trying to receive next request from the requester.
///
/// Can only happen if requester is disconnected/shut down.
#[derive(Debug)]
pub struct RequestRecvError;

impl From<mpsc::RecvError> for RequestRecvError {
    fn from(_: mpsc::RecvError) -> Self {
        Self {}
    }
}

/// Requester half of the channel.
///
/// It is used to send requests to the worker.
#[derive(Debug)]
pub struct ServiceWorkerRequester<Req, Resp> {
    sender: mpsc::SyncSender<Req>,
    receiver: mpsc::Receiver<Resp>,
}

impl<Req, Resp> ServiceWorkerRequester<Req, Resp> {
    pub fn send(&mut self, req: Req) -> Result<(), RequestSendError<Req>> {
        Ok(self.sender.send(req)?)
    }

    pub fn try_recv(&mut self) -> Result<Resp, ResponseTryRecvError> {
        Ok(self.receiver.try_recv()?)
    }
}

#[inline(always)]
fn responder_send<T>(
    sender: &mpsc::SyncSender<T>,
    mio_waker: &Arc<mio::Waker>,
    msg: T,
) -> Result<(), ResponseSendError<T>> {
    sender.send(msg)?;
    let _ = mio_waker.wake();
    Ok(())
}

pub struct ServiceWorkerResponderSender<Resp> {
    sender: mpsc::SyncSender<Resp>,
    mio_waker: Arc<mio::Waker>,
}

impl<Resp> ServiceWorkerResponderSender<Resp> {
    pub fn send(&mut self, resp: Resp) -> Result<(), ResponseSendError<Resp>> {
        responder_send(&self.sender, &self.mio_waker, resp)
    }
}

impl<Resp> Clone for ServiceWorkerResponderSender<Resp> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            mio_waker: self.mio_waker.clone(),
        }
    }
}

/// Responder half of the channel.
///
/// It is used by worker to send responses to the requester.
pub struct ServiceWorkerResponder<Req, Resp> {
    sender: mpsc::SyncSender<Resp>,
    receiver: mpsc::Receiver<Req>,
    mio_waker: Arc<mio::Waker>,
}

impl<Req, Resp> ServiceWorkerResponder<Req, Resp> {
    pub fn sender(&self) -> ServiceWorkerResponderSender<Resp> {
        ServiceWorkerResponderSender {
            sender: self.sender.clone(),
            mio_waker: self.mio_waker.clone(),
        }
    }

    pub fn send(&mut self, resp: Resp) -> Result<(), ResponseSendError<Resp>> {
        responder_send(&self.sender, &self.mio_waker, resp)
    }

    pub fn recv(&mut self) -> Result<Req, RequestRecvError> {
        Ok(self.receiver.recv()?)
    }
}

pub fn worker_channel<Req, Resp>(
    mio_waker: Arc<mio::Waker>,
    bound: usize,
) -> (
    ServiceWorkerRequester<Req, Resp>,
    ServiceWorkerResponder<Req, Resp>,
) {
    let (requester_tx, responder_rx) = mpsc::sync_channel(bound);
    let (responder_tx, requester_rx) = mpsc::sync_channel(bound);

    (
        ServiceWorkerRequester {
            sender: requester_tx,
            receiver: requester_rx,
        },
        ServiceWorkerResponder {
            sender: responder_tx,
            receiver: responder_rx,
            mio_waker,
        },
    )
}
