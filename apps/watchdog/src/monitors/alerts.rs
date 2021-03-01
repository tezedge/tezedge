// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::convert::TryInto;
use std::fmt;
use std::hash::{Hash, Hasher};

use percentage::{Percentage, PercentageInteger};
use slog::{crit, Logger};

use shell::stats::memory::ProcessMemoryStats;

use crate::configuration::AlertThresholds;
use crate::monitors::TEZEDGE_VOLUME_PATH;
use crate::slack::SlackServer;
use crate::ResourceUtilization;

#[derive(Debug, PartialEq)]
pub enum AlertResult {
    Incresed(MonitorAlert),
    Decreased(MonitorAlert),
    Unchanged,
}

#[derive(Clone, Debug)]
pub struct Alerts {
    inner: HashSet<MonitorAlert>,
    memory_threshold: u64,
    disk_threshold: u64,
    node_stuck_threshold: i64,
}

#[derive(Clone, Debug, Eq)]
pub struct MonitorAlert {
    level: AlertLevel,
    kind: AlertKind,
    timestamp: Option<i64>,
    reported: bool,
    value: u64,
}

// implement PartialEq and Hash traits manually to achieve that each kind of alert is only once in the HashSet
impl PartialEq for MonitorAlert {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl Hash for MonitorAlert {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
    }
}

impl MonitorAlert {
    pub fn new(level: AlertLevel, kind: AlertKind, timestamp: Option<i64>, value: u64) -> Self {
        Self {
            level,
            kind,
            timestamp,
            reported: false,
            value,
        }
    }
}

impl Alerts {
    pub fn new(allert_tresholds: AlertThresholds) -> Self {
        Self {
            inner: HashSet::default(),
            memory_threshold: allert_tresholds.ram,
            disk_threshold: allert_tresholds.disk,
            node_stuck_threshold: allert_tresholds.head,
        }
    }

    pub fn assign_resource_alert(
        &mut self,
        kind: AlertKind,
        threshold: u64,
        value: u64,
        timestamp: Option<i64>,
    ) -> AlertResult {
        let level = if value >= AlertLevel::Critical.value().apply_to(threshold) {
            AlertLevel::Critical
        } else if value >= AlertLevel::Severe.value().apply_to(threshold) {
            AlertLevel::Severe
        } else if value >= AlertLevel::Warning.value().apply_to(threshold) {
            AlertLevel::Warning
        } else if value >= AlertLevel::Info.value().apply_to(threshold) {
            AlertLevel::Info
        } else {
            AlertLevel::NonAlert
        };

        let alert = MonitorAlert {
            level,
            kind,
            timestamp,
            reported: false,
            value,
        };

        // decide if the alert has increased/decreased
        if self.inner.contains(&alert) {
            if let Some(previous_alert) = self.inner.get(&alert) {
                if alert.level == AlertLevel::NonAlert {
                    self.inner.remove(&alert);
                    AlertResult::Unchanged
                } else if alert.level > previous_alert.level {
                    self.inner.replace(alert.clone());
                    AlertResult::Incresed(alert)
                } else if alert.level < previous_alert.level {
                    self.inner.replace(alert.clone());
                    AlertResult::Decreased(alert)
                } else {
                    AlertResult::Unchanged
                }
            } else {
                AlertResult::Unchanged
            }
        } else if alert.level != AlertLevel::NonAlert {
            self.inner.insert(alert.clone());
            AlertResult::Incresed(alert)
        } else {
            AlertResult::Unchanged
        }
    }

    pub fn assign_node_stuck_alert(
        &mut self,
        last_checked_head_level: Option<u64>,
        current_head_level: u64,
        current_time: i64,
        log: &Logger,
    ) -> AlertResult {
        if let Some(last_checked_head_level) = last_checked_head_level {
            let head_alert = MonitorAlert::new(
                AlertLevel::Critical,
                AlertKind::NodeStucked,
                Some(current_time),
                current_head_level,
            );

            if let Some(alert) = self.inner.get(&head_alert) {
                // The node is stuck when the level from the last measurement is the same as in the current measurement
                if last_checked_head_level == current_head_level {
                    // report the alert trough slack if it was not already reported
                    if alert.reported {
                        crit!(
                            log,
                            "Node still STUCK, already reported. LEVEL: {}",
                            current_head_level
                        )
                    } else {
                        // check for the threshold
                        // Note: When the node is synced, the blocks update in more or less fixed interval (1min on mainnet)
                        // so do not report the alert until it's time stuck exceeds a defined threshold
                        if current_time - alert.timestamp.unwrap_or(current_time)
                            > self.node_stuck_threshold
                        {
                            crit!(log, "Node STUCK! - LEVEL {}", current_head_level);

                            let mut modified = alert.clone();
                            modified.reported = true;

                            self.inner.replace(modified.clone());
                            return AlertResult::Incresed(modified);
                        } else {
                            crit!(
                                log,
                                "Node appears to be STUCK - LEVEL {}, time until alert: {}s",
                                current_head_level,
                                self.node_stuck_threshold
                                    - (current_time - alert.timestamp.unwrap_or(current_time))
                            );
                        }
                    }
                } else {
                    // When the node apploies the next block, it becomes unstuck, report this
                    crit!(log, "Node unstuck. Level: {}", current_head_level);
                    self.inner.remove(&head_alert);
                    return AlertResult::Decreased(head_alert);
                }
            } else {
                // No alert was reported, node is stuck, insert alert, but do not notify trough slack, lets wait for the treshold
                if last_checked_head_level == current_head_level {
                    crit!(
                        log,
                        "Node appears to be stuck on level {}, time until alert: {}s",
                        current_head_level,
                        self.node_stuck_threshold
                    );
                    self.inner.insert(head_alert.clone());
                }
            }
        }
        AlertResult::Unchanged
    }

    pub async fn check_disk_alert(
        &mut self,
        slack: &SlackServer,
        time: i64,
    ) -> Result<(), failure::Error> {
        // gets the total space on the filesystem of the specified path
        let free_disk_space = fs2::free_space(TEZEDGE_VOLUME_PATH)?;
        // let total_disk_space = fs2::total_space(TEZEDGE_VOLUME_PATH)?;
        let total_disk_space = fs2::total_space(TEZEDGE_VOLUME_PATH)?;

        // set it to a percentage of the max capacity
        let disk_threshold = 100 / self.disk_threshold * total_disk_space;

        let res = self.assign_resource_alert(
            AlertKind::Disk,
            disk_threshold,
            total_disk_space - free_disk_space,
            Some(time),
        );
        send_resource_alert(slack, res).await?;
        Ok(())
    }

    pub async fn check_memory_alert(
        &mut self,
        slack: &SlackServer,
        time: i64,
        last_measurement: ResourceUtilization,
    ) -> Result<(), failure::Error> {
        let ram_total = last_measurement.memory().node().resident_mem()
            + last_measurement
                .memory()
                .protocol_runners()
                .as_ref()
                .unwrap_or(&ProcessMemoryStats::default())
                .resident_mem();
        let res = self.assign_resource_alert(
            AlertKind::Memory,
            self.memory_threshold,
            ram_total.try_into().unwrap_or(0),
            Some(time),
        );

        send_resource_alert(slack, res).await?;

        Ok(())
    }

    pub async fn check_node_stuck_alert(
        &mut self,
        last_checked_head_level: &mut Option<u64>,
        current_head_level: u64,
        current_time: i64,
        slack: &SlackServer,
        log: &Logger,
    ) -> Result<(), failure::Error> {
        match self.assign_node_stuck_alert(
            *last_checked_head_level,
            current_head_level,
            current_time,
            log,
        ) {
            AlertResult::Incresed(alert) => {
                slack
                    .send_message(&format!(
                        ":warning: Node is stuck on level: {}",
                        alert.value
                    ))
                    .await?;
            }
            AlertResult::Decreased(alert) => {
                slack
                    .send_message(&format!(
                        ":information_source: Node is back to applying blocks on level: {}",
                        alert.value
                    ))
                    .await?;
            }
            AlertResult::Unchanged => (/* Do not alert on unchanged */),
        }

        *last_checked_head_level = Some(current_head_level);

        Ok(())
    }

    #[cfg(test)]
    fn contains(&mut self, kind: AlertKind) -> bool {
        // doesn't matter what level or value, just kind
        let alert = MonitorAlert::new(AlertLevel::NonAlert, kind, None, 0);
        self.inner.contains(&alert)
    }

    #[cfg(test)]
    fn get(&mut self, kind: AlertKind) -> Option<&MonitorAlert> {
        // doesn't matter what level or value, just kind
        let alert = MonitorAlert::new(AlertLevel::NonAlert, kind, None, 0);
        self.inner.get(&alert)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd)]
pub enum AlertLevel {
    NonAlert = 1,
    Info = 2,
    Warning = 3,
    Severe = 4,
    Critical = 5,
}

impl AlertLevel {
    pub fn value(&self) -> PercentageInteger {
        match *self {
            AlertLevel::NonAlert => Percentage::from(0),
            AlertLevel::Info => Percentage::from(40),
            AlertLevel::Warning => Percentage::from(60),
            AlertLevel::Severe => Percentage::from(80),
            AlertLevel::Critical => Percentage::from(100),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AlertKind {
    Disk,
    Memory,
    NodeStucked,
}

impl fmt::Display for AlertKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            AlertKind::Disk => write!(f, "Disk"),
            AlertKind::Memory => write!(f, "Memory"),
            AlertKind::NodeStucked => write!(f, "Synchronization"),
        }
    }
}

async fn send_resource_alert(
    slack: &SlackServer,
    alert_result: AlertResult,
) -> Result<(), failure::Error> {
    match alert_result {
        AlertResult::Incresed(alert) => {
            slack
                .send_message(&format!(
                    ":warning: {} surpassed {}% of the defined threshold! Current value: {}",
                    alert.kind,
                    alert.level.value().value(),
                    alert.value
                ))
                .await?;
        }
        AlertResult::Decreased(alert) => {
            slack
                .send_message(&format!(
                    ":information_source: {} Decreased bellow {}% of the defined threshold! Current value: {}",
                    alert.kind,
                    alert.level.value().value(),
                    alert.value
                ))
                .await?;
        }
        AlertResult::Unchanged => (/* Do nothing */),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disk_alert() {
        const TOTAL_DISK_SPACE: u64 = 100_000_000_000;
        let threshold_percentage = 100;

        let threshold = threshold_percentage / 100 * TOTAL_DISK_SPACE;

        let mut alerts = Alerts::new(AlertThresholds {
            disk: threshold,
            ram: 0,
            head: 0,
        });

        alerts.assign_resource_alert(AlertKind::Disk, threshold, 300_000_000_000, None);

        let disk_alert = alerts.get(AlertKind::Disk).unwrap();
        let expected = MonitorAlert {
            kind: AlertKind::Disk,
            level: AlertLevel::Critical,
            timestamp: None,
            reported: false,
            value: 300_000_000_000,
        };

        assert_eq!(disk_alert.level, expected.level);
        drop(disk_alert);

        // Critical
        alerts.assign_resource_alert(AlertKind::Disk, threshold, 100_000_000_001, None);
        let disk_alert = alerts.get(AlertKind::Disk).unwrap();

        assert_eq!(disk_alert.level, expected.level);

        // Severe
        alerts.assign_resource_alert(AlertKind::Disk, threshold, 85_000_000_000, None);
        let disk_alert = alerts.get(AlertKind::Disk).unwrap();
        let expected = MonitorAlert {
            kind: AlertKind::Disk,
            level: AlertLevel::Severe,
            timestamp: None,
            reported: false,
            value: 85_000_000_000,
        };

        assert_eq!(disk_alert, &expected);
        assert_eq!(disk_alert.level, expected.level);
        drop(disk_alert);

        // Warning
        alerts.assign_resource_alert(AlertKind::Disk, threshold, 60_000_000_001, None);
        let disk_alert = alerts.get(AlertKind::Disk).unwrap();
        let expected = MonitorAlert {
            kind: AlertKind::Disk,
            level: AlertLevel::Warning,
            timestamp: None,
            reported: false,
            value: 60_000_000_001,
        };

        assert_eq!(disk_alert, &expected);
        assert_eq!(disk_alert.level, expected.level);
        drop(disk_alert);

        // Info
        alerts.assign_resource_alert(AlertKind::Disk, threshold, 40_000_000_001, None);
        let disk_alert = alerts.get(AlertKind::Disk).unwrap();
        let expected = MonitorAlert {
            kind: AlertKind::Disk,
            level: AlertLevel::Info,
            timestamp: None,
            reported: false,
            value: 40_000_000_001,
        };

        assert_eq!(disk_alert, &expected);
        assert_eq!(disk_alert.level, expected.level);
        drop(disk_alert);

        // NonAlert
        alerts.assign_resource_alert(AlertKind::Disk, threshold, 39_999_999_999, None);

        assert_eq!(alerts.inner.len(), 0);
    }

    #[test]
    fn test_alert_increase_decrease() {
        let memory = 1000;
        let threshold = 10000;

        let mut alerts = Alerts::new(AlertThresholds {
            ram: threshold,
            disk: 0,
            head: 0,
        });

        alerts.assign_resource_alert(AlertKind::Memory, threshold, memory, None);
        assert!(!alerts.contains(AlertKind::Memory));

        // increase memory consuption, still NonAlert
        let memory = 2000;

        // this should replace the memory alert
        alerts.assign_resource_alert(AlertKind::Memory, threshold, memory, None);

        assert!(!alerts.contains(AlertKind::Memory));
        assert_eq!(alerts.inner.len(), 0);

        // increased to Info alert
        let memory = 5000;

        alerts.assign_resource_alert(AlertKind::Memory, threshold, memory, None);

        // still only one alert, the memory alert, now with increased level
        assert!(alerts.contains(AlertKind::Memory));
        assert_eq!(alerts.inner.len(), 1);
        assert_eq!(
            alerts.get(AlertKind::Memory).unwrap().level,
            AlertLevel::Info
        );

        // increased to Critical alert
        let memory = 10001;

        alerts.assign_resource_alert(AlertKind::Memory, threshold, memory, None);

        // still only one alert, the memory alert, now with increased level
        assert!(alerts.contains(AlertKind::Memory));
        assert_eq!(alerts.inner.len(), 1);
        assert_eq!(
            alerts.get(AlertKind::Memory).unwrap().level,
            AlertLevel::Critical
        );

        let memory = 2000;

        alerts.assign_resource_alert(AlertKind::Memory, threshold, memory, None);

        // still only one alert now the level should decrease to NonAlert
        assert!(!alerts.contains(AlertKind::Memory));
        assert_eq!(alerts.inner.len(), 0);
    }

    #[test]
    fn test_multiple_allerts() {
        let memory = 1000;
        let memory_threshold = 10000;

        const TOTAL_DISK_SPACE: u64 = 100_000_000_000;
        let disk_threshold_percentage = 100;

        let disk_threshold = disk_threshold_percentage / 100 * TOTAL_DISK_SPACE;

        let mut alerts = Alerts::new(AlertThresholds {
            ram: memory_threshold,
            disk: disk_threshold,
            head: 0,
        });

        alerts.assign_resource_alert(AlertKind::Memory, memory_threshold, memory, None);
        assert!(!alerts.contains(AlertKind::Memory));

        // increase memory consuption
        let memory = 7500;
        alerts.assign_resource_alert(AlertKind::Memory, memory_threshold, memory, None);
        assert_eq!(alerts.inner.len(), 1);
        assert_eq!(
            alerts.get(AlertKind::Memory).unwrap().level,
            AlertLevel::Warning
        );

        alerts.assign_resource_alert(AlertKind::Disk, disk_threshold, 300_000_000_000, None);

        assert_eq!(alerts.inner.len(), 2);
        assert_eq!(
            alerts.get(AlertKind::Memory).unwrap().level,
            AlertLevel::Warning
        );
        assert_eq!(
            alerts.get(AlertKind::Disk).unwrap().level,
            AlertLevel::Critical
        );

        let memory = 1000;
        let disk = 100_100;

        alerts.assign_resource_alert(AlertKind::Disk, disk_threshold, disk, None);
        alerts.assign_resource_alert(AlertKind::Memory, memory_threshold, memory, None);
        assert_eq!(alerts.inner.len(), 0);
    }

    #[test]
    fn test_node_stuck_alert() {
        let mut alerts = Alerts::new(AlertThresholds {
            disk: 0,
            ram: 0,
            head: 300,
        });

        // discard the logs
        let log = Logger::root(slog::Discard, slog::o!());
        let initial_time: i64 = 1617296614;

        // no alert should generate
        let res = alerts.assign_node_stuck_alert(Some(125), 126, initial_time, &log);
        assert_eq!(alerts.inner.len(), 0);
        assert_eq!(res, AlertResult::Unchanged);

        // 5s in future - register the error, but do not report
        let res = alerts.assign_node_stuck_alert(Some(126), 126, initial_time + 5, &log);
        assert_eq!(alerts.inner.len(), 1);
        assert_eq!(res, AlertResult::Unchanged);

        // 150s in future - error is still registered but the threshold is still not exceeded
        let res = alerts.assign_node_stuck_alert(Some(126), 126, initial_time + 150, &log);
        assert_eq!(alerts.inner.len(), 1);
        assert_eq!(res, AlertResult::Unchanged);

        // 306s in future - error is still registered and now reported
        let mut expected = MonitorAlert::new(
            AlertLevel::Critical,
            AlertKind::NodeStucked,
            Some(initial_time + 306),
            126,
        );
        expected.reported = true;
        let res = alerts.assign_node_stuck_alert(Some(126), 126, initial_time + 306, &log);
        assert_eq!(alerts.inner.len(), 1);
        assert_eq!(res, AlertResult::Incresed(expected));
    }
}
