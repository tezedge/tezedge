use std::fs::File;
use std::io::prelude::*;
use std::io::BufWriter;
use std::sync::mpsc;

use crate::proposals::*;

pub struct ProposalPersisterHandle {
    sender: Option<mpsc::SyncSender<RecordedProposal>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for ProposalPersisterHandle {
    fn drop(&mut self) {
        drop(self.sender.take());
        if let Some(jh) = self.join_handle.take() {
            jh.join().unwrap();
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
        // TODO: better error handling.
        self.sender
            .as_mut()
            .unwrap()
            .send(proposal.into())
            .expect("recorded proposal persister disconnected!");
    }
}

pub struct ProposalPersister {
    channel: mpsc::Receiver<RecordedProposal>,
}

impl ProposalPersister {
    pub fn start() -> ProposalPersisterHandle {
        let (tx, rx) = mpsc::sync_channel(1024);

        let state = Self { channel: rx };

        let join_handle = std::thread::spawn(move || run(state));

        ProposalPersisterHandle {
            sender: Some(tx),
            join_handle: Some(join_handle),
        }
    }
}

fn run(state: ProposalPersister) {
    let file = File::create("/tmp/tezedge/recorded_proposals.log")
        .expect("couldn't open/create file for persisting proposals");
    let mut buf_writer = BufWriter::new(file);

    loop {
        let proposal = match state.channel.recv() {
            Ok(proposal) => proposal,
            Err(_) => break,
        };

        serde_json::to_writer(&mut buf_writer, &proposal).unwrap();
    }
    buf_writer.flush().unwrap();
    buf_writer.into_inner().unwrap().sync_all().unwrap();
}
