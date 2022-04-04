// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Monitors delegate related activities (baking/endorsing) on a particular node

use std::{
    collections::{BTreeMap, HashSet},
    net::SocketAddr,
};

use anyhow::Result;
use crypto::hash::BlockHash;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;
use slog::Logger;
use tezos_messages::p2p::encoding::block_header::BlockHeader;
use tokio::sync::mpsc::{channel, Sender};

use crate::slack::SlackServer;

use super::statistics::{EndorsementOperationSummary, LockedBTreeMap};

#[derive(Debug, Deserialize)]
#[allow(unused)]
struct BakingRights {
    level: i32,
    delegate: String,
    round: u16,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct DelegateEndorsingRights {
    delegate: String,
    first_slot: u16,
    endorsing_power: u16,
}

impl DelegateEndorsingRights {
    pub fn get_first_slot(&self) -> u16 {
        self.first_slot
    }
}

#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct EndorsingRights {
    level: i32,
    pub delegates: Vec<DelegateEndorsingRights>,
}

impl EndorsingRights {
    // pub fn get_delegate(&self, delegate: &str) -> Option<DelegateEndorsingRights> {
    //     self.delegates.into_iter().filter(|delegate_rights| delegate_rights.delegate.eq(delegate)).collect::<Vec<DelegateEndorsingRights>>().last().cloned()
    // }
}

#[derive(Debug, Deserialize)]
pub struct Head {
    hash: BlockHash,
    #[serde(flatten)]
    header: BlockHeader,
}

pub struct DelegatesMonitor {
    node_addr: SocketAddr,
    delegates: Vec<String>,
    endorsmenet_summary_storage: LockedBTreeMap<i32, EndorsementOperationSummary>,
    slack: Option<SlackServer>,
    log: Logger,
}

impl DelegatesMonitor {
    pub fn new(
        node_addr: SocketAddr,
        delegates: Vec<String>,
        endorsmenet_summary_storage: LockedBTreeMap<i32, EndorsementOperationSummary>,
        slack: Option<SlackServer>,
        log: Logger,
    ) -> Self {
        Self {
            node_addr,
            endorsmenet_summary_storage,
            delegates,
            slack,
            log,
        }
    }
    pub async fn run(self) -> anyhow::Result<()> {
        slog::info!(self.log, "Waiting for the node to be bootstrapped...");
        node_null_monitor(self.node_addr, "/monitor/bootstrapped").await?;
        slog::info!(self.log, "The node is bootstrapped.");

        let (tx, mut rx) = channel(1);
        tokio::spawn(node_monitor::<Head, _>(
            self.node_addr,
            "/monitor/heads/main",
            tx,
        ));
        let mut last_level = 0;
        let mut baking_failures = HashSet::new();
        let mut endorsing_failures = HashSet::new();
        while let Some(head_res) = rx.recv().await {
            let (hash, header) = match head_res {
                Ok(Head { hash, header }) => (hash, header),
                Err(err) => {
                    slog::error!(self.log, "error deseriaizing header: {err}");
                    continue;
                }
            };

            let level = header.level();
            slog::debug!(self.log, "new head `{hash}` at level `{level}`");

            if level - 2 > last_level {
                last_level = level - 2;
                if let Err(err) = self
                    .check_block(
                        &hash,
                        last_level,
                        &mut baking_failures,
                        &mut endorsing_failures,
                    )
                    .await
                {
                    slog::warn!(
                        self.log,
                        "error checking delegates for block `{hash}`: `{err}`"
                    );
                }
            }
        }
        Ok(())
    }

    async fn check_block<'a>(
        &'a self,
        hash: &BlockHash,
        level: i32,
        baking_failures: &mut HashSet<&'a String>,
        endorsing_failures: &mut HashSet<&'a String>,
    ) -> anyhow::Result<()> {
        let mut operations = None;
        for delegate in &self.delegates {
            if let Some(round) = self.get_baking_rights(delegate, hash, level).await? {
                slog::debug!(
                    self.log,
                    "Baker `{delegate}` could bake round `{round}` on level `{level}`"
                );
                let block_round = self.get_block_round(level).await?;
                if round <= block_round {
                    let ok = self
                        .check_baker(delegate, round, level, baking_failures.contains(delegate))
                        .await?;
                    if ok {
                        baking_failures.remove(delegate);
                    } else {
                        baking_failures.insert(delegate);
                    }
                }
            }
            if self.get_endorsing_rights(delegate, hash, level).await? {
                slog::debug!(
                    self.log,
                    "Baker `{delegate}` could endorse block on level `{level}`"
                );
                let operations = match &operations {
                    Some(ops) => ops,
                    None => operations.insert(self.get_operations(level).await?),
                };
                let ok = self
                    .check_operation(
                        delegate,
                        level,
                        operations,
                        endorsing_failures.contains(delegate),
                    )
                    .await?;
                if ok {
                    endorsing_failures.remove(delegate);
                } else {
                    endorsing_failures.insert(delegate);
                }
            }
        }
        Ok(())
    }

    async fn get_block_round(&self, level: i32) -> anyhow::Result<u16> {
        node_get::<u16, _>(
            self.node_addr,
            format!("/chains/main/blocks/{level}/helpers/round"),
        )
        .await
    }

    /// For the given block `block`, fetches baking rights for this block level.
    async fn get_baking_rights(
        &self,
        delegate: &str,
        block: &BlockHash,
        level: i32,
    ) -> anyhow::Result<Option<u16>> {
        let round = node_get::<Vec<BakingRights>, _>(
                self.node_addr,
                format!(
                    "/chains/main/blocks/{block}/helpers/baking_rights?level={level}&delegate={delegate}&max_round=4"
                ),
            )
            .await?.into_iter().next().map(|r| r.round);
        Ok(round)
    }

    /// For the given block `block`, fetches endorsing rights for this block level.
    async fn get_endorsing_rights(
        &self,
        delegate: &str,
        block: &BlockHash,
        level: i32,
    ) -> anyhow::Result<bool> {
        let has_endorsing_rights =
            node_get::<Vec<EndorsingRights>, _>(
                self.node_addr,
                format!("/chains/main/blocks/{block}/helpers/endorsing_rights?level={level}&delegate={delegate}"),
            )
            .await?.into_iter().next().map_or(false, |_| true);
        Ok(has_endorsing_rights)
    }

    fn get_baker(value: &Value) -> Option<&str> {
        value.as_object()?.get("baker")?.as_str()
    }

    async fn check_baker(
        &self,
        delegate: &str,
        round: u16,
        level: i32,
        was_failure: bool,
    ) -> anyhow::Result<bool> {
        let metadata = node_get::<Value, _>(
            self.node_addr,
            format!("/chains/main/blocks/{level}/metadata"),
        )
        .await?;
        let baker = Self::get_baker(&metadata)
            .ok_or(anyhow::format_err!("cannot fetch baker from metadata"))?;
        if baker != delegate {
            self.report_error(format!(
                "Lost `{delegate}`'s block at level `{level}`, round `{round}`",
            ));
            Ok(false)
        } else {
            if was_failure {
                self.report_recover(format!(
                    "`{delegate}` baked block at level `{level}` after failure",
                ));
            }
            Ok(true)
        }
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
        level: i32,
        operations: &Vec<Value>,
        was_failure: bool,
    ) -> anyhow::Result<bool> {
        for operation in operations {
            slog::trace!(
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
                    if was_failure {
                        self.report_recover(format!(
                            "`{delegate}` endorsed block on level `{level}` after failure"
                        ));
                    }
                    return Ok(true);
                }
            }
        }
        if let Some(summary) = self.endorsmenet_summary_storage.get(level)? {
            self.report_error(format!(
                "Missed `{delegate}`'s endorsement for level `{level}`\nSummary: {}", summary
            ));
        } else {
            self.report_error(format!(
                "Missed `{delegate}`'s endorsement for level `{level}`\nSummary: Not found"
            ));
        }
        Ok(false)
    }

    async fn get_operations(&self, level: i32) -> anyhow::Result<Vec<Value>> {
        node_get::<Vec<_>, _>(
            self.node_addr,
            format!("/chains/main/blocks/{level}/operations"),
        )
        .await?
        .into_iter()
        .next()
        .ok_or(anyhow::format_err!("Empty operations list"))
    }

    fn report_recover(&self, message: String) {
        slog::info!(self.log, "{}", message);
        if let Some(slack) = &self.slack {
            let slack = slack.clone();
            tokio::spawn(async move {
                slack
                    .send_message(&format!(":white_check_mark: {message}"))
                    .await;
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

pub async fn node_null_monitor<S>(address: SocketAddr, path: S) -> anyhow::Result<()>
where
    S: AsRef<str>,
{
    let mut res = reqwest::get(format!(
        "http://{address}{path}",
        address = address.to_string(),
        path = path.as_ref()
    ))
    .await?;
    while res.chunk().await?.is_some() {}
    Ok(())
}
