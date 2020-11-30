// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::thread;
use std::time::Duration;

use ipc::*;

mod common;

#[test]
fn ipc_fork_and_exchange() -> Result<(), failure::Error> {
    let sock_path = temp_sock();

    let child_pid = common::fork(|| {
        // wait for parent to be ready
        let sock_path = sock_path.as_path();
        while !sock_path.exists() {
            thread::sleep(Duration::from_millis(20));
        }

        let client: IpcClient<String, String> = IpcClient::new(&sock_path);
        let (mut rx, mut tx) = client.connect().unwrap();
        tx.send(&String::from("hello")).unwrap();
        let recv = rx.receive().unwrap();
        assert_eq!(recv, "quick");

        tx.send(&String::from("this is")).unwrap();
        let recv = rx.receive().unwrap();
        assert_eq!(recv, "brown");

        tx.send(&String::from("dog")).unwrap();
        let recv = rx.receive().unwrap();
        assert_eq!(recv, "fox");
    });
    assert!(child_pid > 0);

    let mut server: IpcServer<String, String> = IpcServer::bind_path(&sock_path)?;
    let (mut rx, mut tx) = server.accept().unwrap();
    tx.send(&String::from("quick")).unwrap();
    let recv = rx.receive().unwrap();
    assert_eq!(recv, "hello");

    tx.send(&String::from("brown")).unwrap();
    let recv = rx.receive().unwrap();
    assert_eq!(recv, "this is");

    tx.send(&String::from("fox")).unwrap();
    let recv = rx.receive().unwrap();
    assert_eq!(recv, "dog");

    Ok(())
}

#[test]
fn ipc_fork_and_panic() -> Result<(), failure::Error> {
    let sock_path = temp_sock();

    let child_pid = common::fork(|| {
        // wait for parent to be ready
        let sock_path = sock_path.as_path();
        while !sock_path.exists() {
            thread::sleep(Duration::from_millis(20));
        }

        let client: IpcClient<String, String> = IpcClient::new(&sock_path);
        let (mut rx, mut tx) = client.connect().unwrap();
        tx.send(&String::from("hello")).unwrap();
        let recv = rx.receive().unwrap();
        assert_eq!(recv, "quick");

        panic!("oooops");
    });
    assert!(child_pid > 0);

    let mut server: IpcServer<String, String> = IpcServer::bind_path(&sock_path)?;
    let (mut rx, mut tx) = server.accept().unwrap();
    tx.send(&String::from("quick")).unwrap();
    let recv = rx.receive().unwrap();
    assert_eq!(recv, "hello");

    assert!(rx.receive().is_err());

    Ok(())
}

#[test]
fn ipc_connect_panic() -> Result<(), failure::Error> {
    let sock_path = temp_sock();

    let child_pid = common::fork(|| {
        // wait for parent to be ready
        let sock_path = sock_path.as_path();
        while !sock_path.exists() {
            thread::sleep(Duration::from_millis(20));
        }
        panic!("oooops");
    });
    assert!(child_pid > 0);

    let mut server: IpcServer<String, String> = IpcServer::bind_path(&sock_path)?;
    assert!(server.accept().is_err());

    Ok(())
}
