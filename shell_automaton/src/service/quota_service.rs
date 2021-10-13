use std::{
    sync::{
        mpsc::{sync_channel, SyncSender},
        Arc,
    },
    thread,
    time::Duration,
};

pub trait QuotaService {
    fn schedule(&mut self, timeout: Duration);
}

pub struct QuotaServiceDefault {
    tx: SyncSender<Duration>,
}

impl QuotaServiceDefault {
    pub fn new(waker: Arc<mio::Waker>) -> Self {
        let (tx, rx) = sync_channel::<Duration>(0);
        thread::Builder::new()
            .name("quota-waker-thread".to_string())
            .spawn(move || loop {
                if let Ok(timeout) = rx.recv() {
                    eprintln!("quota update in {}ms", timeout.as_millis());
                    thread::sleep(timeout);
                    eprintln!("quota update now");
                    let _ = waker.wake();
                }
            })
            .unwrap();
        Self { tx }
    }
}

impl QuotaService for QuotaServiceDefault {
    fn schedule(&mut self, timeout: Duration) {
        let _ = self.tx.try_send(timeout);
    }
}
