// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{sync::mpsc, thread, time::SystemTime};

use tenderbake as tb;

use crate::machine::{BakerAction, TickEventAction};

pub struct Timer {
    handle: Option<thread::JoinHandle<()>>,
    task_tx: Option<mpsc::Sender<tb::Timestamp>>,
}

impl Timer {
    pub fn spawn(event_sender: mpsc::Sender<BakerAction>) -> Self {
        let (task_tx, task_rx) = mpsc::channel::<tb::Timestamp>();
        let handle = thread::spawn(move || {
            let mut timeout_duration = None;
            loop {
                let next = match timeout_duration.take() {
                    Some(duration) => match task_rx.recv_timeout(duration) {
                        Ok(next) => next,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            let _ = event_sender.send(BakerAction::TickEvent(TickEventAction {}));
                            continue;
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    },
                    None => match task_rx.recv() {
                        Ok(next) => next,
                        Err(mpsc::RecvError) => break,
                    },
                };
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .expect("the unix epoch has begun");
                if next.unix_epoch > now {
                    timeout_duration = Some(next.unix_epoch - now);
                }
            }
        });

        Timer {
            handle: Some(handle),
            task_tx: Some(task_tx),
        }
    }

    pub fn schedule(&self, timestamp: tb::Timestamp) {
        self.task_tx
            .as_ref()
            .expect("cannot fail")
            .send(timestamp)
            .expect("timer thread running");
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(task_tx) = self.task_tx.take() {
            drop(task_tx);
        }
        if let Some(handle) = self.handle.take() {
            handle.join().expect("failed to stop timer");
        }
    }
}
