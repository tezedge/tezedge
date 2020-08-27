// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]
extern crate test;

use std::path::PathBuf;
use std::process::Child;
use std::thread;
use std::time::Duration;

use failure::format_err;
use serial_test::serial;
use slog::{error, info, Level, Logger, warn};

use tezos_api::environment::{TEZOS_ENV, TezosEnvironmentConfiguration};
use tezos_api::ffi::{InitProtocolContextResult, TezosRuntimeConfiguration};
use tezos_wrapper::{TezosApiConnectionPool, TezosApiConnectionPoolConfiguration};
use tezos_wrapper::service::{ExecutableProtocolRunner, IpcCmdServer, ProtocolEndpointConfiguration, ProtocolRunnerEndpoint};

mod common;

#[ignore]
#[test]
#[serial]
fn test_mutliple_protocol_runners_with_one_write_multiple_read_init_context() -> Result<(), failure::Error> {

    // logger
    let log_level = common::log_level();
    let log = common::create_logger(log_level.clone());
    let number_of_endpoints = 10;

    // We must have just one write and others can be readonly
    // we need to ensure, that first is write context created
    let mut flags_readonly = test_data::init_flags_readonly(number_of_endpoints);

    let context_db_path = PathBuf::from(common::prepare_empty_dir("__shell_test_mutliple_protocol_runners"));

    // spawn thread for init_protocol_context for every endpoint
    let mut handles = Vec::new();
    for i in 0..number_of_endpoints {
        // create endpoint
        let (mut protocol, child, endpoint_name) = create_endpoint(
            log.clone(),
            log_level.clone(),
            format!("test_multiple_endpoint_{}", i),
            context_db_path.clone(),
        )?;

        // choose flag readonly
        let flag_readonly = flags_readonly.pop_front().expect("Every thread should have defined flag!");

        // spawn thread for every endpoint and try to initialize protocol context
        let handle = thread::spawn(move || -> Result<InitProtocolContextResult, failure::Error> {
            // init protocol read or write
            let result = match protocol.accept() {
                Ok(proto) => Ok({
                    if flag_readonly {
                        proto.init_protocol_for_read()?
                    } else {
                        proto.init_protocol_for_write(false, &None)?
                    }
                }),
                Err(e) => Err(format_err!("{:?}", e))
            };
            result
        });
        handles.push((handle, child, endpoint_name, flag_readonly));
    }

    // check result
    assert_eq!(number_of_endpoints as usize, handles.len());
    let mut success_counter = 0;
    for (handle, mut child, endpoint_name, flag_readonly) in handles {
        match handle.join().unwrap() {
            Ok(result) => {
                info!(log, "Init protocol context success"; "endpoint_name" => endpoint_name.clone(), "flag_readonly" => flag_readonly);
                match child.kill() {
                    Ok(_) => (),
                    Err(e) => warn!(log, "Failed to kill child process"; "endpoint_name" => endpoint_name.clone(), "flag_readonly" => flag_readonly, "error" => format!("{:?}", &e))
                };
                if !result.supported_protocol_hashes.is_empty() {
                    success_counter += 1;
                }
            }
            Err(e) => {
                error!(log, "Init protocol context error"; "endpoint_name" => endpoint_name.clone(), "flag_readonly" => flag_readonly, "error" => format!("{:?}", &e));
                match child.kill() {
                    Ok(_) => (),
                    Err(e) => warn!(log, "Failed to kill child process"; "endpoint_name" => endpoint_name.clone(), "flag_readonly" => flag_readonly, "error" => format!("{:?}", &e))
                };
            }
        }
    }

    Ok(assert_eq!(number_of_endpoints, success_counter))
}

fn create_endpoint(log: Logger, log_level: Level, name: String, context_db_path: PathBuf) -> Result<(IpcCmdServer, Child, String), failure::Error> {

    // environement
    let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV.get(&test_data::TEZOS_NETWORK).expect("no environment configuration");

    // init protocol runner endpoint
    let protocol_runner = common::protocol_runner_executable_path();
    let protocol_runner_endpoint = ProtocolRunnerEndpoint::<ExecutableProtocolRunner>::new(
        &name,
        ProtocolEndpointConfiguration::new(
            TezosRuntimeConfiguration {
                log_enabled: common::is_ocaml_log_enabled(),
                no_of_ffi_calls_treshold_for_gc: common::no_of_ffi_calls_treshold_for_gc(),
                debug_mode: false,
            },
            tezos_env.clone(),
            false,
            &context_db_path,
            &protocol_runner,
            log_level.clone(),
            false,
        ),
        log.clone(),
    );

    // start subprocess
    let (subprocess, protocol_commands, endpoint_name, ..) = match protocol_runner_endpoint.start() {
        Ok(subprocess) => {
            let ProtocolRunnerEndpoint {
                commands,
                name,
                ..
            } = protocol_runner_endpoint;
            (subprocess, commands, name)
        }
        Err(e) => panic!("Error to start test_protocol_runner_endpoint: {} - error: {:?}", protocol_runner.as_os_str().to_str().unwrap_or("-none-"), e)
    };

    Ok((protocol_commands, subprocess, endpoint_name))
}

#[ignore]
#[test]
#[serial]
fn test_readonly_protocol_runner_connection_pool() -> Result<(), failure::Error> {
    // logger
    let log_level = common::log_level();
    let log = common::create_logger(log_level.clone());
    let number_of_endpoints = 3;

    // environement
    let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV.get(&test_data::TEZOS_NETWORK).expect("no environment configuration");

    // storage
    let context_db_path = PathBuf::from(common::prepare_empty_dir("__shell_test_readonly_protocol_runner_pool"));

    // init protocol runner endpoint
    let protocol_runner = common::protocol_runner_executable_path();

    // at first we need to create one writerable context, because of creating new one - see feature AT_LEAST_ONE_WRITE_PROTOCOL_CONTEXT_WAS_SUCCESS_AT_FIRST_LOCK
    let (mut write_context_commands, ..) = create_endpoint(log.clone(), log_level.clone(), format!("test_one_writeable_endpoint"), context_db_path.clone())?;
    let genesis_context_hash = write_context_commands
        .accept()?
        .init_protocol_for_write(true, &None)?
        .genesis_commit_hash
        .expect("Genesis context_hash should be commited!");

    // cfg for pool
    let pool_cfg = TezosApiConnectionPoolConfiguration {
        min_connections: 0,
        max_connections: number_of_endpoints,
        connection_timeout: Duration::from_secs(3),
        max_lifetime: Duration::from_secs(60),
        idle_timeout: Duration::from_secs(60),
    };

    // cfg for protocol runner
    let endpoint_cfg = ProtocolEndpointConfiguration::new(
        TezosRuntimeConfiguration {
            log_enabled: common::is_ocaml_log_enabled(),
            no_of_ffi_calls_treshold_for_gc: common::no_of_ffi_calls_treshold_for_gc(),
            debug_mode: false,
        },
        tezos_env.clone(),
        false,
        &context_db_path,
        &protocol_runner,
        log_level.clone(),
        false,
    );

    // create pool
    let pool_wrapper = TezosApiConnectionPool::new_with_readonly_context(
        "test_pool_with_readonly_context".to_string(),
        pool_cfg,
        endpoint_cfg,
        log,
    );

    // create readonly pool pool
    let pool = &pool_wrapper.pool;
    assert_eq!(0, pool.state().connections);

    // test pool
    {
        {
            // acquire one
            assert!(&pool.get()?.api.genesis_result_data(&genesis_context_hash).is_ok());
            assert_eq!(1, pool.state().connections);
            // released1
        }
        assert_eq!(1, pool.state().connections);

        // acquire (reuse connection and not release - api is not out of the scope yet)
        let api1 = &pool.get()?.api;
        assert!(api1.genesis_result_data(&genesis_context_hash).is_ok());
        assert_eq!(1, pool.state().connections);

        // acquire (create new connection and not release - api is not out of the scope yet)
        let api2 = &pool.get()?.api;
        assert!(api2.genesis_result_data(&genesis_context_hash).is_ok());
        assert_eq!(2, pool.state().connections);

        // acquire (create new connection and not release - api is not out of the scope yet)
        let api3 = &pool.get()?.api;
        assert!(api3.genesis_result_data(&genesis_context_hash).is_ok());
        assert_eq!(3, pool.state().connections);

        // try get another one (reached max number of connections)
        assert!(pool.try_get().is_none());
        assert_eq!(3, pool.state().connections);

        // get another one (reached max number of connections) - default connection_timeout
        let result = pool.get();
        assert!(result.is_err())

        // release all api1/2/3
    }
    assert_eq!(3, pool.state().connections);
    assert_eq!(3, pool.state().idle_connections);

    // acquire (reused and not release)
    let api = &pool.get()?.api;
    assert!(api.genesis_result_data(&genesis_context_hash).is_ok());
    assert_eq!(3, pool.state().connections);
    assert_eq!(2, pool.state().idle_connections);

    Ok(())
}

mod test_data {
    use std::collections::VecDeque;

    use rand::Rng;

    use tezos_api::environment::TezosEnvironment;

    pub const TEZOS_NETWORK: TezosEnvironment = TezosEnvironment::Carthagenet;

    /// Initialize all to readonly true and one set randomly to as 'write/false'
    pub fn init_flags_readonly(count: i32) -> VecDeque<bool> {
        // initialize all to readonly true
        let mut flags_readonly = VecDeque::new();

        // and one set randomly to as 'write/false'
        let mut rng = rand::thread_rng();
        let rand_index = rng.gen_range(0, count);

        for i in 0..count {
            if rand_index == i {
                flags_readonly.push_back(false);
            } else {
                flags_readonly.push_back(true);
            }
        }

        assert_eq!(flags_readonly.len(), count as usize);
        let mut count_false = 0;
        for f in &flags_readonly {
            if !f {
                count_false += 1;
            }
        }
        assert_eq!(count_false, 1);
        flags_readonly
    }
}