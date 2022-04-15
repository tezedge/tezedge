// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::collections::VecDeque;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use monitors::delegate::DelegatesMonitor;
use slog::{error, info, Drain, Level, Logger};
use tokio::signal;
use tokio::time::{sleep, Duration};

use netinfo::Netinfo;

mod configuration;
mod display_info;
mod monitors;
mod node;
mod rpc;
mod slack;

use crate::configuration::DeployMonitoringEnvironment;
use crate::monitors::alerts::Alerts;
use crate::monitors::resource::{ResourceMonitor, ResourceUtilization, ResourceUtilizationStorage};
use crate::rpc::MEASUREMENTS_MAX_CAPACITY;

const PROCESS_LOOKUP_INTERVAL: Duration = Duration::from_secs(10);

#[tokio::main]
async fn main() {
    let env = configuration::DeployMonitoringEnvironment::from_args();

    // create an slog logger
    let log = create_logger(env.log_level);

    let DeployMonitoringEnvironment {
        slack_configuration,
        tezedge_alert_thresholds,
        ocaml_alert_thresholds,
        resource_monitor_interval,
        ..
    } = env.clone();

    let slack_server =
        slack_configuration.map(|cfg| slack::SlackServer::new(cfg.slack_url, log.clone()));

    let mut storages = Vec::new();

    if env.wait_for_nodes {
        for mut node in env.nodes.clone() {
            while node.pid().is_none() {
                if let Some(pid) =
                    node::find_node_process_id(node.port()).expect("cannot get process id")
                {
                    info!(log, "Found node with port {} -> PID: {}", node.port(), pid);
                    node.set_pid(Some(pid));
                    let resource_storage = ResourceUtilizationStorage::new(
                        node.clone(),
                        Arc::new(RwLock::new(VecDeque::<ResourceUtilization>::with_capacity(
                            MEASUREMENTS_MAX_CAPACITY,
                        ))),
                    );
                    storages.push(resource_storage);
                } else {
                    info!(
                        log,
                        "Waiting for node {} with port: {}",
                        node.tag(),
                        node.port()
                    );
                    sleep(PROCESS_LOOKUP_INTERVAL).await;
                }
            }
        }
    } else {
        for mut node in env.nodes.clone() {
            if let Some(pid) =
                node::find_node_process_id(node.port()).expect("cannot get process id")
            {
                info!(log, "Found node with port {} -> PID: {}", node.port(), pid);
                node.set_pid(Some(pid));
                let resource_storage = ResourceUtilizationStorage::new(
                    node.clone(),
                    Arc::new(RwLock::new(VecDeque::<ResourceUtilization>::with_capacity(
                        MEASUREMENTS_MAX_CAPACITY,
                    ))),
                );
                storages.push(resource_storage);
            } else {
                panic!("Cannot find defined node with port {}", node.port())
            }
        }
    }

    let net_interfaces = if let Ok(net_interfaces) = Netinfo::list_net_interfaces() {
        net_interfaces
    } else {
        panic!("No network interfaces found!");
    };

    let mut netinfo = match Netinfo::new(&net_interfaces[..]) {
        Ok(netinfo) => netinfo,
        Err(e) => panic!("Failed to create netinfo struct, reason: {}", e),
    };

    if let Err(e) = netinfo.set_min_refresh_interval(Some(Duration::from_secs(1))) {
        panic!("Cannot set min_refresh_interval for netinfo, reason: {}", e)
    }

    if let Some(delegates) = env.delegates {
        for node in env.nodes {
            let node_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), node.port());
            slog::info!(
                log,
                "Using node `{node}` to monitor performance for delegates",
                node = node.tag()
            );
            let delegates = delegates.clone();
            let log = log.clone();
            let slack = slack_server.clone();
            let stats_dir = env.stats_dir.clone();
            tokio::spawn(async move {
                if let Err(err) = DelegatesMonitor::new(
                    node_addr,
                    delegates,
                    slack,
                    env.report_each_error,
                    stats_dir,
                    log.clone(),
                )
                .run()
                .await
                {
                    slog::error!(log, "Error in delegates monitor: `{err}`");
                }
            });
        }
    }

    if !storages.is_empty() {
        let alerts = Alerts::new(tezedge_alert_thresholds, ocaml_alert_thresholds);

        if let Err(e) = netinfo.start() {
            panic!("Cannot start netinfo, reason: {}", e)
        }

        netinfo.clear().expect("Cannot clear netinfo");

        let mut resource_monitor = ResourceMonitor::new(
            storages.clone(),
            HashMap::new(),
            alerts,
            log.clone(),
            slack_server.clone(),
            netinfo,
        );

        let thread_log = log.clone();
        let handle = tokio::spawn(async move {
            // wait for the first refresh, so it doesn't offset the first measurement
            sleep(Duration::from_secs(resource_monitor_interval)).await;
            loop {
                if let Err(e) = resource_monitor.take_measurement().await {
                    error!(thread_log, "Resource monitoring error: {}", e);
                }
                sleep(Duration::from_secs(resource_monitor_interval)).await;
            }
        });

        info!(log, "Starting rpc server on port {}", &env.rpc_port);
        let rpc_server_handle = rpc::spawn_rpc_server(env.rpc_port, log.clone(), storages);

        if let Some(slack) = slack_server.as_ref() {
            slack.send_message("Monitoring started").await;
            slack
                .send_message(&format!(
                    "Alert thresholds for tezedge nodes set to: {}",
                    tezedge_alert_thresholds
                ))
                .await;
            slack
                .send_message(&format!(
                    "Alert thresholds for ocaml nodes set to: {}",
                    ocaml_alert_thresholds
                ))
                .await;
        }

        let mut signal_stream =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Unable to handle SIGTERM");

        tokio::select! {
            s = signal::ctrl_c() => {
                s.expect("Failed to listen for ctrl-c event");
                info!(log, "Ctrl-c or SIGINT received!");
            }
            _ = signal_stream.recv() => {
                info!(log, "SIGTERM received!");
            }
        }
        if let Some(slack) = slack_server {
            slack.send_message(":skull: Monitoring stopped").await;
        }

        // drop the looping thread handles (forces exit)
        drop(handle);
        drop(rpc_server_handle);
    } else {
        panic!("No nodes found to monitor!");
    }
}

/// Creates a slog Logger
fn create_logger(level: Level) -> Logger {
    let drain = slog_async::Async::new(
        slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
            .build()
            .fuse(),
    )
    .chan_size(32768)
    .overflow_strategy(slog_async::OverflowStrategy::Block)
    .build()
    .filter_level(level)
    .fuse();
    Logger::root(drain, slog::o!())
}
