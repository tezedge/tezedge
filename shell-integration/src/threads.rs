use std::sync::Arc;
// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

pub type ShutdownError = anyhow::Error;
pub type ShutdownCallback = Box<dyn Fn() -> Result<(), ShutdownError> + Send>;
pub type ThreadRunningStatus = Arc<AtomicBool>;

pub struct ThreadWatcher {
    thread_name: String,
    thread_running_status: ThreadRunningStatus,
    thread: Option<JoinHandle<()>>,
    shutdown_triggered_ok: Arc<AtomicBool>,
    shutdown_callback: ShutdownCallback,
}

impl ThreadWatcher {
    pub fn start(thread_name: String, shutdown_callback: ShutdownCallback) -> Self {
        Self {
            thread_name,
            thread_running_status: Arc::new(AtomicBool::new(true)),
            thread: None,
            shutdown_triggered_ok: Arc::new(AtomicBool::new(false)),
            shutdown_callback,
        }
    }

    pub fn stop(&self) -> Result<(), ShutdownError> {
        if self.shutdown_triggered_ok.load(Ordering::Acquire) {
            return Ok(());
        }
        self.thread_running_status.store(false, Ordering::Release);
        let shutdown_result = (self.shutdown_callback)();
        if shutdown_result.is_ok() {
            self.shutdown_triggered_ok.store(true, Ordering::Release)
        }
        shutdown_result
    }

    pub fn thread_name(&self) -> &str {
        &self.thread_name
    }

    pub fn thread_running_status(&self) -> &ThreadRunningStatus {
        &self.thread_running_status
    }

    pub fn set_thread(&mut self, thread: JoinHandle<()>) {
        self.thread = Some(thread);
    }

    pub fn thread(&mut self) -> Option<JoinHandle<()>> {
        self.thread.take()
    }
}

impl Drop for ThreadWatcher {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            eprintln!(
                "Failed to stop thread watcher for thread_name: {}, reason: {:?}",
                self.thread_name, e
            );
        }
    }
}
