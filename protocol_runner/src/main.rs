// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! Separate Tezos protocol runner, as we used OCaml protocol more and more, we noticed increasing
//! problems, from panics to high memory usage, for better stability, we separated protocol into
//! self-contained process communicating through Unix Socket.

use clap::{App, Arg};
use slog::*;

extern crate jemallocator;

#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn create_logger(log_level: Level, endpoint_name: String) -> Logger {
    let drain = slog_async::Async::new(
        slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
            .build()
            .fuse(),
    )
    .build()
    .filter_level(log_level)
    .fuse();

    Logger::root(drain, slog::o!("endpoint" => endpoint_name))
}

fn main() {
    let matches = App::new("TezEdge Protocol Runner")
        .version(env!("CARGO_PKG_VERSION"))
        .author("TezEdge and the project contributors")
        .about("Tezos Protocol Runner")
        .arg(
            Arg::with_name("socket-path")
                .short("c")
                .long("socket-path")
                .value_name("path")
                .help("Path to a command socket")
                .takes_value(true)
                .empty_values(false)
                .required(true),
        )
        .arg(
            Arg::with_name("endpoint")
                .long("endpoint")
                .value_name("STRING")
                .help("Name of the endpoint, which spawned runner")
                .takes_value(true)
                .empty_values(false)
                .required(true),
        )
        .arg(
            Arg::with_name("log-level")
                .long("log-level")
                .takes_value(true)
                .value_name("LEVEL")
                .possible_values(&["critical", "error", "warn", "info", "debug", "trace"])
                .help("Set log level"),
        )
        .get_matches();

    let cmd_socket_path = matches
        .value_of("socket-path")
        .expect("Missing socket-path value");
    let endpoint_name = matches
        .value_of("endpoint")
        .expect("Missing endpoint value")
        .to_string();
    let log_level = matches
        .value_of("log-level")
        .unwrap_or("info")
        .parse::<slog::Level>()
        .expect("Was expecting one value from slog::Level");

    let log = create_logger(log_level, endpoint_name);

    let shutdown_callback = |log: &Logger| {
        debug!(log, "Shutting down OCaml runtime");
        match std::panic::catch_unwind(|| {
            tezos_interop::shutdown();
        }) {
            Ok(_) => debug!(log, "OCaml runtime shutdown was successful"),
            Err(e) => {
                warn!(log, "Shutting down OCaml runtime failed (check running sub-process for this endpoint or `[protocol-runner] <defunct>`, and and terminate/kill manually)!"; "reason" => format!("{:?}", e))
            }
        }
    };

    {
        let log = log.clone();
        // do nothing and wait for parent process to send termination command
        // this is just fallback, if ProtocolController.shutdown will fail or if we need to kill sub-process manually
        ctrlc::set_handler(move || {
            shutdown_callback(&log);
            warn!(log, "Protocol runner was terminated/killed/ctrl-c - please, check running sub-processes for `[protocol-runner] <defunct>`, and terminate/kill manually!");
        }).expect("Error setting Ctrl-C handler");
    }

    // Process commands from from the Rust node. Most commands are instructions for the Tezos protocol
    //if let Err(err) = ipc_loop::process_protocol_commands::<crate::tezos::NativeTezosLib, _, _>(
    //    cmd_socket_path,
    //    &log,
    //    shutdown_callback,
    //) {
    //    error!(log, "Error while processing protocol commands"; "reason" => format!("{:?}", err));
    //    shutdown_callback(&log);
    //}
    // TODO: error handling
    tezos_interop::start_ipc_loop(cmd_socket_path.into());

    info!(log, "Protocol runner finished gracefully");
}
