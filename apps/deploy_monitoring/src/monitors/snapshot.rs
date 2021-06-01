// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use chrono::Utc;
use fs_extra::dir::{copy, create_all, CopyOptions};
use slog::{error, info, Logger};

use crate::constants::{TEZEDGE_PORT, TEZEDGE_VOLUME_PATH};
use crate::node::{Node, TezedgeNode};
use crate::slack::SlackServer;

pub struct SnapshotMonitor {
    log: Logger,
    compose_file_path: PathBuf,
    snapshot_levels: Vec<u64>,
    slack_server: Option<SlackServer>,
}

impl SnapshotMonitor {
    pub fn new(
        log: Logger,
        compose_file_path: PathBuf,
        snapshot_levels: Vec<u64>,
        slack_server: Option<SlackServer>,
    ) -> Self {
        Self {
            log,
            compose_file_path,
            snapshot_levels,
            slack_server,
        }
    }

    fn take_snapshot(&self, current_level: &u64) -> Result<(), failure::Error> {
        info!(self.log, "Taking snapshot, level: {}", current_level);
        if let Some(slack) = &self.slack_server {
            slack.send_message(&format!("Taking snapshot, level: {}", current_level));
        }
        let options = CopyOptions::new();
        let destination = format!(
            "/usr/local/etc/tezedge-data/tezedge_{}_{}",
            current_level,
            Utc::now().to_rfc3339()
        );
        info!(self.log, "Creating snapshot direcotry");
        if let Err(e) = create_all(&destination, false) {
            error!(
                self.log,
                "Cannot create directory path {}, error: {}", destination, e
            );
        }

        info!(self.log, "Copying data...");
        if let Err(e) = copy(TEZEDGE_VOLUME_PATH, &destination, &options) {
            error!(
                self.log,
                "Failed to copy the tezedge data to path {}, error: {}", destination, e
            );
        }
        Ok(())
    }

    pub async fn monitor_snapshotting(&mut self) -> Result<bool, failure::Error> {
        let current_head_info = TezedgeNode::collect_head_data(TEZEDGE_PORT).await?;

        // get one level to check for
        if let Some(snapshot_level) = self.snapshot_levels.get(0) {
            println!("Snapshot level for check: {}", snapshot_level);
            if *current_head_info.level() >= snapshot_level - 100 {
                info!(self.log, "Stopping node to take a snapshot");
                match TezedgeNode::stop_node(&self.compose_file_path) {
                    Ok(()) => info!(self.log, "Node stopped succesfully"),
                    Err(e) => error!(self.log, "Node failed to stop; Err: {}", e),
                }
                self.take_snapshot(current_head_info.level())?;
                match TezedgeNode::start_node(&self.compose_file_path, &self.log).await {
                    Ok(()) => info!(self.log, "Node started succesfully"),
                    Err(e) => error!(self.log, "Node failed to start; Err: {}", e),
                }
                self.snapshot_levels.remove(0);
            }
            Ok(false)
        } else {
            // TODO - snapshotting
            Ok(true)
        }
    }
}
