// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Monitors delegate related activities (baking/endorsing) on a particular node

use std::{collections::HashMap, net::SocketAddr};

use anyhow::Result;
use crypto::hash::BlockHash;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;
use slog::Logger;
use tezos_messages::p2p::encoding::block_header::BlockHeader;
use tokio::sync::mpsc::{channel, Sender};

use crate::slack::SlackServer;

#[derive(Debug, Deserialize)]
#[allow(unused)]
struct BakingRights {
    level: i32,
    delegate: String,
    round: u16,
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
struct DelegateEndorsingRights {
    delegate: String,
    first_slot: u16,
    endorsing_power: u16,
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
struct EndorsingRights {
    level: i32,
    delegates: Vec<DelegateEndorsingRights>,
}

#[derive(Debug, Deserialize)]
pub struct Head {
    hash: BlockHash,
    #[serde(flatten)]
    header: BlockHeader,
}

pub struct DelegateRightsForBlock {
    /// If the delegate is eligible for baking the block with this level (e.g. current head).
    baker_rights: Option<i32>,
    /// If the delegate is eligible for endorsing the block with this level (e.g. the current head's predecessor).
    endorser_rights: Option<i32>,
}

pub struct DelegateFailures {
    baking: bool,
    endorsing: bool,
}

pub struct DelegatesMonitor {
    node_addr: SocketAddr,
    delegates: Vec<String>,
    slack: Option<SlackServer>,
    log: Logger,
}

impl DelegatesMonitor {
    pub fn new(
        node_addr: SocketAddr,
        delegates: Vec<String>,
        slack: Option<SlackServer>,
        log: Logger,
    ) -> Self {
        Self {
            node_addr,
            delegates,
            slack,
            log,
        }
    }
    pub async fn run(self) -> anyhow::Result<()> {
        let (tx, mut rx) = channel(1);
        tokio::spawn(node_monitor::<Head, _>(
            self.node_addr,
            "/monitor/heads/main",
            tx,
        ));
        let mut failures = DelegateFailures { baking: false, endorsing: false };
        while let Some(head_res) = rx.recv().await {
            let (hash, header) = match head_res {
                Ok(Head { hash, header }) => (hash, header),
                Err(err) => {
                    slog::error!(self.log, "error deseriaizing header: {err}");
                    continue;
                }
            };

            slog::debug!(self.log, "new head: `{hash}`");
            if let Err(err) = self.check_block(&hash, &header, &mut failures).await {
                slog::warn!(
                    self.log,
                    "error checking delegates for block `{hash}`: `{err}`"
                );
            }
        }
        Ok(())
    }

    async fn check_block<'a>(
        &'a self,
        hash: &BlockHash,
        header: &BlockHeader,
        failures: &mut DelegateFailures,
    ) -> anyhow::Result<()> {
        let rights = self
            .fetch_rights_for_delegates(&hash, header.level())
            .await?;
        let mut operations = None;
        for (
            delegate,
            DelegateRightsForBlock {
                baker_rights,
                endorser_rights,
            },
        ) in rights
        {
            if let Some(level) = baker_rights {
                self.check_baker(delegate, &hash, level, &mut failures.baking).await?
            }
            if let Some(level) = endorser_rights {
                let operations = match &operations {
                    Some(ops) => ops,
                    None => operations.insert(self.get_operations(hash).await?),
                };
                self.check_operation(delegate, &hash, level, operations, &mut failures.endorsing)
                    .await?;
            }
        }
        Ok(())
    }

    async fn fetch_rights_for_delegates<'a>(
        &'a self,
        block: &BlockHash,
        level: i32,
    ) -> anyhow::Result<HashMap<&'a str, DelegateRightsForBlock>> {
        let mut rights = HashMap::new();
        for delegate in &self.delegates {
            let next = self.fetch_rights(delegate, block, level).await?;
            rights.insert(delegate.as_str(), next);
        }
        Ok(rights)
    }

    /// For the given block `block`, fetches baking rights for this block level
    /// and endorsing rights for the previous block.
    async fn fetch_rights(
        &self,
        delegate: &str,
        block: &BlockHash,
        level: i32,
    ) -> anyhow::Result<DelegateRightsForBlock> {
        let baker_rights = {
            let rights: Vec<BakingRights> = node_get(
                self.node_addr,
                format!(
                    "/chains/main/blocks/{block}/helpers/baking_rights?level={level}&delegate={delegate}&max_round=0"
                ),
            )
            .await?;
            if rights.len() > 0 {
                Some(level)
            } else {
                None
            }
        };
        let endorser_rights = {
            if let Some(level) = level.checked_sub(1) {
                let rights: Vec<EndorsingRights> = node_get(
                    self.node_addr,
                    format!("/chains/main/blocks/{block}/helpers/endorsing_rights?level={level}&delegate={delegate}"),
                )
                .await?;
                if rights.len() > 0 {
                    Some(level)
                } else {
                    None
                }
            } else {
                None
            }
        };
        Ok(DelegateRightsForBlock {
            baker_rights,
            endorser_rights,
        })
    }

    fn get_baker(value: &Value) -> Option<&str> {
        value.as_object()?.get("baker")?.as_str()
    }

    async fn check_baker(
        &self,
        delegate: &str,
        hash: &BlockHash,
        level: i32,
        failure: &mut bool,
    ) -> anyhow::Result<()> {
        slog::debug!(self.log, "checking that `{hash}` is signed by baker");
        let metadata = node_get::<Value, _>(
            self.node_addr,
            format!("/chains/main/blocks/{hash}/metadata"),
        )
        .await?;
        let baker = Self::get_baker(&metadata)
            .ok_or(anyhow::format_err!("cannot fetch baker from metadata"))?;
        if baker != delegate {
            self.report_error(format!("Lost `{delegate}`'s block at level `{level}`",));
            *failure = true;
        } else if *failure {
            self.report_recover(format!("`{delegate}` baked block at level `{level}` after failure",));
            *failure = false;
        }
        Ok(())
    }

    fn get_operation_contents(value: &Value) -> Option<&Vec<Value>> {
        value.as_object()?.get("contents")?.as_array()
    }

    fn get_operation_contents_kind(value: &Value) -> Option<&str> {
        value.as_object()?.get("kind")?.as_str()
    }

    fn get_operation_contents_delegate(value: &Value) -> Option<&str> {
        value
            .as_object()?
            .get("metadata")?
            .as_object()?
            .get("delegate")?
            .as_str()
    }

    async fn check_operation(
        &self,
        delegate: &str,
        hash: &BlockHash,
        level: i32,
        operations: &Vec<Value>,
        failure: &mut bool,
    ) -> anyhow::Result<()> {
        for operation in operations {
            slog::debug!(
                self.log,
                "checking {delegate} against operation {operation:?}"
            );
            let contents = match Self::get_operation_contents(operation)
                .ok_or(anyhow::format_err!("cannot get operation contents"))?
                .split_first()
            {
                Some((first, [])) => first,
                _ => continue,
            };

            if "endorsement"
                == Self::get_operation_contents_kind(contents).ok_or(anyhow::format_err!(
                    "cannot get kind from operation contents"
                ))?
            {
                if delegate
                    == Self::get_operation_contents_delegate(contents).ok_or(
                        anyhow::format_err!("cannot get delegate from operation contents metadata"),
                    )?
                {
                    if *failure {
                        self.report_recover(format!("`{delegate}` endorsed block on level `{level}` in block `{hash}` after failure"));
                        *failure = false;
                    }
                    return Ok(());
                }
            }
        }
        self.report_error(format!(
            "Missed `{delegate}`'s endorsement for level `{level}` in block `{hash}`"
        ));
        *failure = true;

        Ok(())
    }

    async fn get_operations(&self, hash: &BlockHash) -> anyhow::Result<Vec<Value>> {
        let operations = node_get::<Vec<_>, _>(
            self.node_addr,
            format!("/chains/main/blocks/{hash}/operations"),
        )
        .await?;
        operations
            .into_iter()
            .next()
            .ok_or(anyhow::format_err!("Empty operations list"))
    }

    fn report_recover(&self, message: String) {
        slog::info!(self.log, "{}", message);
        if let Some(slack) = &self.slack {
            let slack = slack.clone();
            tokio::spawn(async move {
                slack.send_message(&format!(":white_check_mark: {message}")).await;
            });
        }
    }

    fn report_error(&self, message: String) {
        slog::crit!(self.log, "{}", message);
        if let Some(slack) = &self.slack {
            let slack = slack.clone();
            tokio::spawn(async move {
                slack.send_message(&format!(":warning: {message}")).await;
            });
        }
    }
}

pub async fn node_get<T, S>(address: SocketAddr, path: S) -> anyhow::Result<T>
where
    T: DeserializeOwned,
    S: AsRef<str>,
{
    //eprintln!(">>> {path}", path = path.as_ref());
    let resp = reqwest::get(format!(
        "http://{address}{path}",
        address = address.to_string(),
        path = path.as_ref()
    ))
    .await?;
    let json = resp.json().await?;
    //eprintln!("<<< {json}");
    let value = serde_json::from_value(json)?;
    Ok(value)
}

pub async fn node_monitor<T, S>(
    address: SocketAddr,
    path: S,
    sender: Sender<Result<T, serde_json::Error>>,
) -> anyhow::Result<()>
where
    T: 'static + DeserializeOwned + std::fmt::Debug + Send + Sync,
    S: AsRef<str>,
{
    let mut res = reqwest::get(format!(
        "http://{address}{path}",
        address = address.to_string(),
        path = path.as_ref()
    ))
    .await?;
    while let Some(chunk) = res.chunk().await? {
        let json: Value = serde_json::from_slice(&chunk)?;
        //eprintln!("<<< {json}");
        let value = serde_json::from_value(json);
        sender.send(value).await?;
    }
    Ok(())
}
