use serde_json::de::IoRead;
use serde_json::StreamDeserializer;
use std::fs::File;
use std::io::BufReader;

use crate::proposals::*;

pub struct ProposalLoader {
    reader: StreamDeserializer<'static, IoRead<BufReader<File>>, RecordedProposal>,
}

impl ProposalLoader {
    pub fn new() -> Self {
        let file = File::open("/tmp/tezedge/recorded_proposals.log")
            .expect("couldn't open file for loading proposals");
        let reader = serde_json::Deserializer::from_reader(BufReader::new(file)).into_iter();
        Self { reader }
    }

    pub fn next(&mut self) -> Option<Result<RecordedProposal, serde_json::Error>> {
        self.reader.next()
    }
}

impl Clone for ProposalLoader {
    fn clone(&self) -> Self {
        // TODO
        unimplemented!("this is temporary, this type shouldn't be clonable")
    }
}
