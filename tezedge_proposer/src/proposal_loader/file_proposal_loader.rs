// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde_json::de::IoRead;
use serde_json::StreamDeserializer;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use tezedge_state::proposals::RecordedProposal;

pub struct FileProposalLoader {
    reader: StreamDeserializer<'static, IoRead<BufReader<File>>, RecordedProposal>,
}

impl FileProposalLoader {
    pub fn new<P>(file_name: P) -> Self
    where
        P: AsRef<Path>,
    {
        let file = File::open(file_name).expect("couldn't open file for loading proposals");
        let reader = serde_json::Deserializer::from_reader(BufReader::new(file)).into_iter();
        Self { reader }
    }
}

impl Iterator for FileProposalLoader {
    type Item = Result<RecordedProposal, failure::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.next().map(|x| Ok(x?))
    }
}
