// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use reqwest::Url;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct Arguments {
    #[structopt(long)]
    pub base_dir: PathBuf,
    #[structopt(long)]
    pub endpoint: Url,
    #[structopt(short)]
    pub archive: bool,
    #[structopt(subcommand)]
    pub command: Command,
}

#[derive(StructOpt, Debug)]
pub enum Command {
    RunWithLocalNode {
        #[structopt(long)]
        node_dir: PathBuf,
        #[structopt(long)]
        baker: String,
    },
}

impl Arguments {
    pub fn from_args() -> Self {
        StructOpt::from_args()
    }
}
