use std::{sync::Arc, thread, time::Duration};

use slog::{error, Logger};

pub trait QuotaService {}

pub struct QuotaServiceDefault;

impl QuotaServiceDefault {
    pub fn new(waker: Arc<mio::Waker>, timeout: Duration, log: Logger) -> Self {
        thread::Builder::new()
            .name("quota-waker-thread".to_string())
            .spawn(move || {
                loop {
                    thread::sleep(timeout);
                    if let Err(err) = waker.wake() {
                        error!(log, "Failed to wake MIO waker, timeouts will be missing"; "error" => err.to_string());
                    }
                }
            })
            .unwrap();
        Self
    }
}

impl QuotaService for QuotaServiceDefault {}
