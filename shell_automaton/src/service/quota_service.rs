use std::{
    collections::VecDeque,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use mio::Waker;
use slog::{error, Logger};

pub trait QuotaService {
    fn schedule(&mut self, timeout: Duration);
}

pub struct QuotaServiceDefault {
    tx: Sender<Instant>,
}

impl QuotaServiceDefault {
    pub fn new(waker: Arc<mio::Waker>, log: Logger) -> Self {
        let (tx, rx) = channel::<Instant>();
        let timeouts = TimeoutQueue::new(rx, waker, log);
        thread::Builder::new()
            .name("quota-waker-thread".to_string())
            .spawn(move || timeouts.run())
            .unwrap();
        Self { tx }
    }
}

impl QuotaService for QuotaServiceDefault {
    fn schedule(&mut self, timeout: Duration) {
        let _ = self.tx.send(Instant::now() + timeout);
    }
}

struct TimeoutQueue {
    timeouts: VecDeque<Instant>,
    rx: Receiver<Instant>,
    waker: Arc<Waker>,
    log: Logger,
}

impl TimeoutQueue {
    fn new(rx: Receiver<Instant>, waker: Arc<Waker>, log: Logger) -> Self {
        Self {
            timeouts: VecDeque::new(),
            rx,
            waker,
            log,
        }
    }

    fn run(mut self) {
        loop {
            let index_past_now = match self.timeouts.binary_search(&Instant::now()) {
                Ok(i) => i + 1,
                Err(i) => i,
            };
            if index_past_now > 0 {
                if let Err(err) = self.waker.wake() {
                    error!(self.log, "Failed to wake MIO waker, timeouts will be missing"; "error" => err.to_string());
                }
                self.timeouts.drain(..index_past_now);
            }
            match self.timeouts.back().cloned() {
                Some(instant) => match self.rx.recv_deadline(instant) {
                    Ok(new_instant) => {
                        if let Err(index) = self.timeouts.binary_search(&new_instant) {
                            self.timeouts.insert(index, new_instant);
                        }
                    }
                    Err(_) => (),
                },
                None => match self.rx.recv() {
                    Ok(new_instant) => {
                        if let Err(index) = self.timeouts.binary_search(&new_instant) {
                            self.timeouts.insert(index, new_instant);
                        }
                    }
                    Err(err) => {
                        error!(self.log, "Failed to receive from channel"; "error" => err.to_string())
                    }
                },
            }
        }
    }
}
