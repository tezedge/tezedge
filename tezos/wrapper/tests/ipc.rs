// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::thread;
use std::time::Duration;

use tokio::runtime::current_thread::Runtime;

use protocol_wrapper::ipc::*;

mod common;


#[test]
fn fork_and_exchange() -> Result<(), failure::Error> {
    let sock_path = temp_sock();

    let child_pid = common::fork(|| {

        // wait for parent to be ready
        let sock_path = sock_path.as_path();
        while !sock_path.exists() {
            thread::sleep(Duration::from_millis(20));
        }

        let client: IpcClient<String, String> = IpcClient::new(&sock_path);
        let mut rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let (mut rx, mut tx) = client.connect().await.unwrap();
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

    let mut server: IpcServer<String, String> = IpcServer::bind_path(&sock_path)?;


    let mut rt = Runtime::new().unwrap();
    rt.block_on(async move {
        let (mut rx, mut tx) = server.accept().await.unwrap();
        tx.send(&String::from("quick")).await.unwrap();
        let recv = rx.receive().await.unwrap();
        assert_eq!(recv, "hello");

        tx.send(&String::from("brown")).await.unwrap();
        let recv = rx.receive().await.unwrap();
        assert_eq!(recv, "this is");

        tx.send(&String::from("fox")).await.unwrap();
        let recv = rx.receive().await.unwrap();
        assert_eq!(recv, "dog");
    });

    Ok(())
}

#[test]
fn fork_and_panic() -> Result<(), failure::Error> {
    let sock_path = temp_sock();

    let child_pid = common::fork(|| {

        // wait for parent to be ready
        let sock_path = sock_path.as_path();
        while !sock_path.exists() {
            thread::sleep(Duration::from_millis(20));
        }

        let client: IpcClient<String, String> = IpcClient::new(&sock_path);
        let mut rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let (mut rx, mut tx) = client.connect().await.unwrap();
            tx.send(&String::from("hello")).await.unwrap();
            let recv = rx.receive().await.unwrap();
            assert_eq!(recv, "quick");

            panic!("oooops");
        });

    });
    assert!(child_pid > 0);

    let mut server: IpcServer<String, String> = IpcServer::bind_path(&sock_path)?;


    let mut rt = Runtime::new().unwrap();
    rt.block_on(async move {
        let (mut rx, mut tx) = server.accept().await.unwrap();
        tx.send(&String::from("quick")).await.unwrap();
        let recv = rx.receive().await.unwrap();
        assert_eq!(recv, "hello");

        assert!(rx.receive().await.is_err());
    });

    Ok(())
}

