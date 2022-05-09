// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Monitors delegate related activities (baking/endorsing) on a particular node

use std::{
    collections::{BTreeMap, HashMap},
    net::SocketAddr,
    time::Duration,
};

use anyhow::{Context, Result};
use crypto::hash::BlockHash;
use itertools::Itertools;
use reqwest::Response;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};
use slog::Logger;
use tezos_messages::p2p::encoding::block_header::BlockHeader;
use tokio::{
    io::AsyncWriteExt,
    sync::mpsc::{channel, Sender},
};

use crate::slack::SlackServer;

use super::statistics::{FinalEndorsementSummary, LockedBTreeMap};

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
    pub delegate: String,
    pub first_slot: u16,
    pub endorsing_power: u16,
}

impl DelegateEndorsingRights {
    pub fn get_first_slot(&self) -> u16 {
        self.first_slot
    }
}

#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct EndorsingRights {
    pub level: i32,
    pub delegates: Vec<DelegateEndorsingRights>,
}

impl EndorsingRights {
    // pub fn get_delegate(&self, delegate: &str) -> Option<DelegateEndorsingRights> {
    //     self.delegates.into_iter().filter(|delegate_rights| delegate_rights.delegate.eq(delegate)).collect::<Vec<DelegateEndorsingRights>>().last().cloned()
    // }

    pub fn endorsement_powers(&self) -> BTreeMap<u16, u16> {
        self.delegates
            .iter()
            .map(|delegate| (delegate.first_slot, delegate.endorsing_power))
            .collect()
    }
}

#[derive(Debug, Deserialize)]
pub struct Head {
    hash: BlockHash,
    #[serde(flatten)]
    header: BlockHeader,
}

pub struct DelegatesMonitor {
    node_addr: SocketAddr,
    explorer_url: Option<String>,
    delegates: Vec<String>,
    endorsmenet_summary_storage: LockedBTreeMap<i32, FinalEndorsementSummary>,
    slack: Option<SlackServer>,
    each_failure: bool,
    stats_dir: Option<String>,
    log: Logger,
}

impl DelegatesMonitor {
    pub fn new(
        node_addr: SocketAddr,
        explorer_url: Option<String>,
        delegates: Vec<String>,
        endorsmenet_summary_storage: LockedBTreeMap<i32, FinalEndorsementSummary>,
        slack: Option<SlackServer>,
        each_failure: bool,
        stats_dir: Option<String>,
        log: Logger,
    ) -> Self {
        Self {
            node_addr,
            explorer_url,
            delegates,
            endorsmenet_summary_storage,
            slack,
            each_failure,
            stats_dir,
            log,
        }
    }
    pub async fn run(self) -> anyhow::Result<()> {
        loop {
            if let Err(err) = self.run_loop().await {
                slog::warn!(self.log, "Error monitoring delegates: `{err}`, restarting");
                let _ = tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    async fn run_loop(&self) -> anyhow::Result<()> {
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
        let mut baking_failures = HashMap::new();
        let mut endorsing_failures = HashMap::new();
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
                        self.each_failure,
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
        each_failure: bool,
        baking_failures: &mut HashMap<&'a String, usize>,
        endorsing_failures: &mut HashMap<&'a String, usize>,
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
                        .check_baker(
                            delegate,
                            round,
                            level,
                            each_failure,
                            baking_failures.get(delegate),
                        )
                        .await?;
                    if ok {
                        baking_failures.remove(delegate);
                    } else {
                        *baking_failures.entry(delegate).or_insert(0) += 1;
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
                    None => operations.insert(self.get_operations(level + 1).await?),
                };
                let ok = self
                    .check_operation(
                        delegate,
                        level,
                        operations,
                        each_failure,
                        endorsing_failures.get(delegate),
                    )
                    .await?;
                if ok {
                    endorsing_failures.remove(delegate);
                } else {
                    *endorsing_failures.entry(delegate).or_insert(0) += 1;
                    self.on_missed_endorsement(level).await?;
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
        each_failure: bool,
        prev_failures: Option<&usize>,
    ) -> anyhow::Result<bool> {
        let metadata = node_get::<Value, _>(
            self.node_addr,
            format!("/chains/main/blocks/{level}/metadata"),
        )
        .await?;
        let baker = Self::get_baker(&metadata)
            .ok_or(anyhow::format_err!("cannot fetch baker from metadata"))?;
        if baker != delegate {
            if each_failure || prev_failures.is_none() {
                self.report_error(format!(
                    "Lost `{delegate}`'s block at level `{level}`, round `{round}`",
                ));
            }
            Ok(false)
        } else {
            if let Some(prev_failures) = prev_failures {
                self.report_recover(format!(
                    "`{delegate}` baked block at level `{level}` after `{prev_failures}` failures",
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
        each_failure: bool,
        prev_failures: Option<&usize>,
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
                    if let Some(prev_failures) = prev_failures {
                        self.report_recover(format!(
                            "`{delegate}` endorsed block on level `{level}` after `{prev_failures}` failure(s)"
                        ));
                        if let Some(summary) = self.endorsmenet_summary_storage.get(level)? {
                            self.report_recover(format!(
                                "`{delegate}` endorsed block on level `{level}` after `{prev_failures}` failure(s)\nSummary:\n {summary}"
                            ));
                        } else {
                            self.report_recover(format!(
                                "`{delegate}` endorsed block on level `{level}` after `{prev_failures}` failure(s)\nSummary: Not found"
                            ));
                        }
                    }
                    return Ok(true);
                }
            }
        }
        if each_failure || prev_failures.is_none() {
            let levels = ((level - 2)..(level + 2)).join(",");
            let path = format!(
                "/dev/shell/automaton/actions_stats_for_blocks?level={}",
                levels
            );
            let summary = self.endorsmenet_summary_storage.get(level)?;
            let mut action_stats = node_get::<Value, _>(self.node_addr, path).await?;

            let summary = format!(
                "*Summary*: {}",
                summary.map_or("`Error(Not Found)`".to_owned(), |s| s.to_string())
            );
            let action_stats_body = action_stats
                .as_array_mut()
                .map(|v| std::mem::take(v))
                .unwrap_or_default()
                .into_iter()
                .filter_map(|b| {
                    let missed_endorsement_level = level;
                    let level = b.get("block_level")?.as_i64()?;
                    let round = b.get("block_round")?.as_i64()?;
                    let cpu_idle = b.get("cpu_idle")?.as_u64()?;
                    let cpu_busy = b.get("cpu_busy")?.as_u64()?;
                    let bold = match level == missed_endorsement_level as i64 {
                        true => "*",
                        false => "",
                    };
                    Some(format!(
                        "{}level: {} round: {} - cpu_idle: {:.3}s, cpu_busy: {:.3}s{}",
                        bold,
                        level,
                        round,
                        (cpu_idle as f64) / 1_000_000_000.0,
                        (cpu_busy as f64) / 1_000_000_000.0,
                        bold,
                    ))
                })
                .join("\n");
            let action_stats_explorer_link = self
                .explorer_url
                .as_ref()
                .map_or("`Error(Missing Explorer Url)`".to_owned(), |explorer_url| {
                    format!("{}/#/resources/state/{}", explorer_url, level)
                });
            let action_stats_header = format!("*Action Stats:* {action_stats_explorer_link}");
            let action_stats = format!("{action_stats_header}\n{action_stats_body}");

            self.report_error(format!(
                "Missed `{delegate}`'s endorsement for level `{level}`\n\n{summary}\n\n{action_stats}{}",
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

    async fn on_missed_endorsement(&self, level: i32) -> anyhow::Result<()> {
        let stats_dir = if let Some(p) = &self.stats_dir {
            p
        } else {
            return Ok(());
        };
        let path = format!("/dev/shell/automaton/stats/current_head/application?level={level}");
        slog::debug!(self.log, "fetching block application stats using {path}");
        let application = node_get::<Value, _>(self.node_addr, path).await?;
        let blocks = application
            .as_array()
            .ok_or_else(|| anyhow::format_err!("array expected"))?;
        for block in blocks {
            let round = if let Some(round) = block
                .as_object()
                .and_then(|b| b.get("round"))
                .and_then(|round| round.as_i64())
            {
                round
            } else {
                continue;
            };

            let base_time = if let Some(b) = block
                .as_object()
                .and_then(|b| b.get("receive_timestamp"))
                .and_then(|round| round.as_i64())
            {
                b
            } else {
                continue;
            };

            let path = format!("/dev/shell/automaton/endorsements_status?level={level}&round={round}&base_time={base_time}",);
            slog::debug!(self.log, "fetching endorsements using {path}");
            let endorsements = node_get::<Value, _>(self.node_addr, path).await?;
            let path = format!("/dev/shell/automaton/preendorsements_status?level={level}&round={round}&base_time={base_time}",);
            slog::debug!(self.log, "fetching preendorsements using {path}");
            let preendorsements = node_get::<Value, _>(self.node_addr, path).await?;
            let hashes = if let Some(e) = endorsements.as_object() {
                e.keys().cloned().collect::<Vec<_>>()
            } else {
                continue;
            };
            let operation_stats = if !hashes.is_empty() {
                let path = format!(
                    "/dev/shell/automaton/mempool/operation_stats?hash={hashes}",
                    hashes = hashes.join(",")
                );
                node_get::<Value, _>(self.node_addr, path).await?
            } else {
                json!([])
            };

            for (name, json) in [
                ("application", block),
                ("endorsements", &endorsements),
                ("preendorsements", &preendorsements),
                ("operations", &operation_stats),
            ] {
                let mut file =
                    tokio::fs::File::create(&format!("{stats_dir}/{level}-{round}-{name}.json"))
                        .await?;
                file.write_all(json.to_string().as_bytes()).await?;
            }
        }

        let path = format!("/dev/shell/automaton/actions_stats_for_blocks");
        let action_stats = node_get::<Value, _>(self.node_addr, path).await?;

        let mut file = tokio::fs::File::create(&format!("{level}-action_stats.json")).await?;
        file.write_all(action_stats.to_string().as_bytes()).await?;

        Ok(())
    }
}

pub async fn node_get<T, S>(address: SocketAddr, path: S) -> anyhow::Result<T>
where
    T: DeserializeOwned,
    S: AsRef<str> + std::fmt::Display,
{
    let json = node_get_raw(address, &path)
        .await?
        .json()
        .await
        .context(format!("JSONifying `{path}`"))?;
    let value = serde_json::from_value(json)
        .context(format!("deserializing `{path}` response from JSON"))?;
    Ok(value)
}

pub async fn node_get_raw<S>(address: SocketAddr, path: S) -> anyhow::Result<Response>
where
    S: AsRef<str> + std::fmt::Display,
{
    let response = reqwest::get(format!(
        "http://{address}{path}",
        address = address.to_string(),
        path = path.as_ref()
    ))
    .await
    .context(format!("error while fetching `{path}`"))?;
    Ok(response)
}

pub async fn node_monitor<T, S>(
    address: SocketAddr,
    path: S,
    sender: Sender<Result<T, serde_json::Error>>,
) -> anyhow::Result<()>
where
    T: 'static + DeserializeOwned + std::fmt::Debug + Send + Sync,
    S: AsRef<str> + std::fmt::Display,
{
    let mut res = node_get_raw(address, path).await?;
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

#[cfg(test)]
mod tests {
    use std::net::ToSocketAddrs;

    use super::*;
    #[tokio::test]
    #[ignore = "Test for specific failure, might be used later with different parameters"]
    async fn test() {
        let log = crate::create_logger(slog::Level::Debug);
        let monitor = DelegatesMonitor::new(
            "mempool.tezedge.com:28732"
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap(),
            None,
            vec!["tz1Qm727PrLHPme6gcz2Gg8YAXqUrq8oDhio".to_string()],
            LockedBTreeMap::new(),
            None,
            false,
            None,
            log,
        );

        let block_hash =
            BlockHash::from_base58_check("BMG2JyPzyHRj75Mn3p7tdZF8Myz2tURP32vvvDqB4gEFFHBjVGy")
                .unwrap();
        let level = 451_738;

        monitor
            .check_block(
                &block_hash,
                level,
                false,
                &mut HashMap::new(),
                &mut HashMap::new(),
            )
            .await
            .unwrap();
    }
}
