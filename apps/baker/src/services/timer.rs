// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{sync::mpsc, thread, time::SystemTime};

use tenderbake as tb;

use crate::machine::{BakerAction, TickEventAction};

pub struct Timer {
    handle: Option<thread::JoinHandle<()>>,
    task_tx: Option<mpsc::Sender<(tb::Timestamp, i32, i32)>>,
}

impl Timer {
    pub fn spawn(event_sender: mpsc::Sender<BakerAction>) -> Self {
        let (task_tx, task_rx) = mpsc::channel::<(tb::Timestamp, i32, i32)>();
        let handle = thread::spawn(move || {
            let mut timeout_duration = None;
            let mut scheduled_at_level = 0;
            let mut scheduled_at_round = 0;
            loop {
                let (next, l, r) = match timeout_duration.take() {
                    Some(duration) => match task_rx.recv_timeout(duration) {
                        Ok(next) => next,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            let _ = event_sender.send(BakerAction::TickEvent(TickEventAction {
                                scheduled_at_level,
                                scheduled_at_round,
                            }));
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
                    scheduled_at_level = l;
                    scheduled_at_round = r;
                } else {
                    let _ = event_sender.send(BakerAction::TickEvent(TickEventAction {
                        scheduled_at_level: l,
                        scheduled_at_round: r,
                    }));
                }
            }
        });

        Timer {
            handle: Some(handle),
            task_tx: Some(task_tx),
        }
    }

    pub fn schedule(&self, timestamp: tb::Timestamp, level: i32, round: i32) {
        self.task_tx
            .as_ref()
            .expect("cannot fail")
            .send((timestamp, level, round))
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

#[cfg(test)]
mod tests {
    use std::{
        sync::mpsc,
        thread,
        time::{Duration, SystemTime},
    };
    use tenderbake as tb;

    use crate::machine::BakerAction;

    use super::Timer;

    fn new_timer_and_collector() -> (Timer, tb::Timestamp, thread::JoinHandle<Vec<BakerAction>>) {
        let (tx, rx) = mpsc::channel();
        let timer = Timer::spawn(tx);
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("the unix epoch has begun");
        let timestamp = tb::Timestamp { unix_epoch: now };

        let collector = thread::spawn(move || {
            let mut actions = vec![];
            while let Ok(action) = rx.recv() {
                actions.push(action);
            }
            actions
        });

        (timer, timestamp, collector)
    }

    #[test]
    fn timer_basic() {
        let (timer, timestamp, collector) = new_timer_and_collector();

        timer.schedule(timestamp + Duration::from_millis(1000), 0, 0);
        thread::sleep(Duration::from_millis(2500));
        drop(timer);

        let actions = collector.join().unwrap();
        assert_eq!(actions.len(), 1);
    }

    // next request should eclipse previous
    #[test]
    fn timer_eclipse() {
        let (timer, timestamp, collector) = new_timer_and_collector();

        timer.schedule(timestamp + Duration::from_millis(1500), 0, 0);
        timer.schedule(timestamp + Duration::from_millis(1000), 0, 0);
        thread::sleep(Duration::from_millis(2500));
        drop(timer);

        let actions = collector.join().unwrap();
        assert_eq!(actions.len(), 1);
    }

    // should emit action even if it is late
    #[test]
    fn timer_late() {
        let (timer, timestamp, collector) = new_timer_and_collector();

        thread::sleep(Duration::from_millis(1000));
        timer.schedule(timestamp, 0, 0);
        thread::sleep(Duration::from_millis(1000));
        drop(timer);

        let actions = collector.join().unwrap();
        assert_eq!(actions.len(), 1);
    }
}
