// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fs::File;
use std::io::BufWriter;
use std::sync::{mpsc, Arc};

use slog::Logger;

use crate::proposals::*;

/// Try to serialize as json or return it's debug information.
fn stringify_recorded_proposal(proposal: &RecordedProposal) -> String {
    serde_json::to_string(&proposal).unwrap_or_else(|_| format!("{:?}", proposal))
}

pub struct ProposalPersisterHandle {
    log: Logger,
    sender: Option<mpsc::SyncSender<Arc<RecordedProposal>>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for ProposalPersisterHandle {
    fn drop(&mut self) {
        drop(self.sender.take());
        if let Some(jh) = self.join_handle.take() {
            let _ = jh.join();
        }
    }
}

impl Clone for ProposalPersisterHandle {
    fn clone(&self) -> Self {
        // TODO
        unimplemented!("this is temporary, this type shouldn't be clonable")
    }
}

impl ProposalPersisterHandle {
    pub fn persist<P>(&mut self, proposal: P)
    where
        P: Into<RecordedProposal>,
    {
        if let Some(sender) = self.sender.as_mut() {
            let proposal = Arc::new(proposal.into());
            if let Err(_) = sender.send(proposal.clone()) {
                slog::error!(
                    &self.log,
                    "Failed to send proposal for persisting";
                    "reason" => "proposal-persister disconnected",
                    "proposal" => stringify_recorded_proposal(&*proposal));
            }
        }
    }
}

pub struct ProposalPersister {
    log: Logger,
    file: File,
    channel: mpsc::Receiver<Arc<RecordedProposal>>,
}

impl ProposalPersister {
    /// # Panics
    /// Must be called only on startup of the node since it panics.
    pub fn start(log: Logger) -> ProposalPersisterHandle {
        let (tx, rx) = mpsc::sync_channel(1024);

        let file = File::create("/tmp/tezedge/recorded_proposals.log")
            .expect("couldn't open/create file for persisting proposals");

        let state = Self {
            file,
            log: log.clone(),
            channel: rx,
        };

        let join_handle = std::thread::Builder::new()
            .name("proposal-persister".to_owned())
            .spawn(move || run(state))
            .expect("failed to spawn proposal-persister thread");

        ProposalPersisterHandle {
            log,
            sender: Some(tx),
            join_handle: Some(join_handle),
        }
    }
}

fn run(state: ProposalPersister) {
    let log = state.log;
    let mut buf_writer = BufWriter::new(state.file);

    for index in 0u64.. {
        let proposal = match state.channel.recv() {
            Ok(proposal) => proposal,
            Err(mpsc::RecvError) => {
                slog::info!(&log, "Shutting down proposal-persister thread"; "reason" => "sender disconnected");
                break;
            }
        };

        if let Err(err) = serde_json::to_writer(&mut buf_writer, &*proposal) {
            slog::error!(
                &log,
                "Failed to persist proposal";
                "error" => format!("{:?}", err),
                "proposal_index" => index,
                "proposal" => stringify_recorded_proposal(&*proposal))
        }
    }
    match buf_writer.into_inner() {
        Ok(inner) => {
            if let Err(err) = inner.sync_all() {
                slog::error!(&log, "Failed to sync_all proposal persister"; "error" => err);
            }
        }
        Err(err) => {
            slog::error!(&log, "Failed to flush recorded proposals"; "error" => format!("{:?}", err));
        }
    }
    slog::info!(&log, "Thread proposal-persister shut down"; "reason" => "sender disconnected");
}
