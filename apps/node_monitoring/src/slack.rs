// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::{error, info, Logger};

#[derive(Clone)]
pub struct SlackServer {
    monitor_channel_url: String,
    auth_token: String,
    channel: String,
    log: Logger,
}

impl SlackServer {
    pub fn new(
        monitor_channel_url: String,
        auth_token: String,
        channel: String,
        log: Logger,
    ) -> Self {
        Self {
            monitor_channel_url,
            auth_token,
            channel,
            log,
        }
    }

    pub async fn send_message(&self, text: &str) {
        let client = reqwest::Client::new();

        let res = client
            .post(&self.monitor_channel_url)
            .json(&serde_json::json!({ "text": text }))
            .send()
            .await;

        match res {
            Ok(_) => info!(self.log, "Slack message sent: {}", text),
            Err(e) => error!(self.log, "Slack message error: {:?}", e),
        }
    }
}
