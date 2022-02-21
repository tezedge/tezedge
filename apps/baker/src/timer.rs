// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{sync::mpsc, thread, any::Any, time::{SystemTime, Duration}};

use serde::{Serialize, Deserialize};

use crate::machine::action::{Action, TimeoutAction};

pub struct Timer {
    task_sender: mpsc::Sender<Task>,
    handle: thread::JoinHandle<()>,
}

impl Timer {
    pub fn spawn(action_sender: mpsc::Sender<Action>) -> Self {
        let (task_sender, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let mut d = None::<Duration>;
            loop {
                let next = match d.take() {
                    Some(d) => {
                        match rx.recv_timeout(d) {
                            Ok(next) => next,
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                let action = TimeoutAction { now_timestamp_millis: 0 };
                                let _ = action_sender.send(Action::Timeout(action));
                                continue;
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    },
                    None => {
                        match rx.recv() {
                            Ok(next) => next,
                            Err(mpsc::RecvError) => break,
                        }
                    }
                };
                match next {
                    Task::First(timestamp) => {
                        d = Some(timestamp.duration_from_now());
                    }
                    Task::Next(duration) => {
                        d = Some(duration);
                    }
                }
            }
        });
        Timer { task_sender, handle }
    }

    #[allow(dead_code)]
    pub fn first_timeout<F>(&self, timestamp: Timestamp, wrapper: F)
    where
        F: Fn(TimeoutAction) -> Action + Send + 'static,
    {
        let _ = wrapper;
        let _ = self.task_sender.send(Task::First(timestamp));
    }

    #[allow(dead_code)]
    pub fn next_timeout<F>(&self, duration: Duration, wrapper: F)
    where
        F: Fn(TimeoutAction) -> Action + Send + 'static,
    {
        let _ = wrapper;
        let _ = self.task_sender.send(Task::Next(duration));
    }

    pub fn join(self) -> Result<(), Box<dyn Any + Send + 'static>> {
        drop(self.task_sender);
        self.handle.join()
    }
}

enum Task {
    First(Timestamp),
    Next(Duration),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub fn duration_from_now(&self) -> Duration {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("the unix epoch has begun");

        Duration::from_secs(self.0) - now
    }

    pub fn now() -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("the unix epoch has begun");
        Timestamp(now.as_secs())
    }
}
