// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::thread;
use std::time::Duration;

use anyhow::format_err;
use serial_test::serial;

use async_ipc::*;

mod common;

#[test]
#[serial]
#[ignore] // Disabled for now, this crate is deprecated
fn ipc_fork_and_client_exchange() -> Result<(), anyhow::Error> {
    let sock_path = temp_sock();
    assert!(!sock_path.exists());

    let child_pid = common::fork(|| {
        let tokio_runtime = common::create_tokio_runtime();
        tokio_runtime.block_on(async {
            // wait for socket/bind to be ready
            let sock_path = sock_path.as_path();
            while !sock_path.exists() {
                thread::sleep(Duration::from_millis(20));
            }

            // try connect
            let client: IpcClient<String, String> = IpcClient::new(&sock_path);
            let (mut rx, mut tx) = client.connect().await.unwrap();

            // test send/receive
            tx.send(&String::from("hello")).await.unwrap();
            let recv = rx.receive().await.unwrap();
            assert_eq!(recv, "quick");

            tx.send(&String::from("this is")).await.unwrap();
            let recv = rx.receive().await.unwrap();
            assert_eq!(recv, "brown");

            tx.send(&String::from("dog")).await.unwrap();
            let recv = rx.receive().await.unwrap();
            assert_eq!(recv, "fox");
        });
    });
    assert!(child_pid > 0);

    let tokio_runtime = common::create_tokio_runtime();

    tokio_runtime.block_on(async {
        // bind and accept server
        let mut server: IpcServer<String, String> = IpcServer::bind_path(&sock_path).unwrap();
        let (mut rx, mut tx) = server.try_accept(Duration::from_secs(10)).await.unwrap();

        // test send/receive
        tx.send(&String::from("quick")).await.unwrap();
        let recv = rx.receive().await.unwrap();
        assert_eq!(recv, "hello");

        tx.send(&String::from("brown")).await.unwrap();
        let recv = rx.receive().await.unwrap();
        assert_eq!(recv, "this is");

        tx.send(&String::from("fox")).await.unwrap();
        let recv = rx.receive().await.unwrap();
        assert_eq!(recv, "dog");

        assert!(common::wait(child_pid));
    });

    Ok(())
}

#[test]
#[serial]
#[ignore] // Disabled for now, this crate is deprecated
fn ipc_accept_timeout() -> Result<(), anyhow::Error> {
    let tokio_runtime = common::create_tokio_runtime();

    tokio_runtime
        .block_on(async {
            let mut server: IpcServer<String, String> = IpcServer::bind_path(&temp_sock())?;
            let result = server.try_accept(Duration::from_secs(2));

            match result.await {
                Err(IpcError::AcceptTimeout { .. }) => Ok(()),
                Err(e) => Err(format_err!("Unexpected result: {:?}", e)),
                Ok(_) => Err(format_err!("Unexpected result")),
            }
        })
        .unwrap();

    Ok(())
}
