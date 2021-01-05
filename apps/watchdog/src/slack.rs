// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use reqwest::header::AUTHORIZATION;
use slack_hook::{PayloadBuilder, Slack};
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

    pub async fn send_message(&self, text: &str) -> Result<(), failure::Error> {
        // slack webhook url
        let slack = Slack::new(self.monitor_channel_url.as_ref()).unwrap();

        let payload = PayloadBuilder::new()
            // .channel("#monitoring")
            .channel(&self.channel)
            .text(text)
            .build()
            .unwrap();

        // send message
        let resposne = slack.send(&payload);
        match resposne {
            Ok(()) => info!(self.log, "Slack message sent: {}", text),
            Err(e) => error!(self.log, "Slack message error: {:?}", e),
        }

        Ok(())
    }

    pub async fn upload_file(&self, text: &str, file_content: &str) -> Result<(), failure::Error> {
        let params = [
            ("initial_comment", text),
            ("content", file_content),
            ("channels", &self.channel),
        ];

        let client = reqwest::Client::new();

        let res = client
            .post("https://slack.com/api/files.upload")
            .header(AUTHORIZATION, self.auth_token.clone())
            .form(&params)
            .send()
            .await?;

        info!(
            self.log,
            "Slack file upload response: {}",
            res.text().await?
        );
        Ok(())
    }
}
