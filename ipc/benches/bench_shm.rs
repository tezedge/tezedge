// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use bencher::Bencher;
use std::io;
use std::thread;
use std::time::Duration;

use bencher::benchmark_group;
use bencher::benchmark_main;
use rand::Rng;
use serde::{Deserialize, Serialize};

use ipc::{temp_sock, IpcClient, IpcServer};

#[derive(Serialize, Deserialize)]
struct BenchData {
    block_hash: Vec<u8>,
    chain_hash: Vec<u8>,
}

fn fork<F: FnOnce()>(child_func: F) -> libc::pid_t {
    unsafe {
        match libc::fork() {
            -1 => panic!("fork failed: {}", io::Error::last_os_error()),
            0 => {
                child_func();
                libc::exit(0);
            }
            pid => pid,
        }
    }
}

fn rand_file_name() -> String {
    let path = temp_sock();
    path.as_os_str().to_str().unwrap().to_string()
}

fn bench_shm(b: &mut Bencher) {
    use ipmpsc::*;
    let name_parent = rand_file_name();
    let name_child = rand_file_name();
    let buffer_parent = SharedRingBuffer::create(&name_parent, 32 * 1024 * 1024).unwrap();
    let buffer_child = SharedRingBuffer::create(&name_child, 32 * 1024 * 1024).unwrap();

    let child_pid = fork(|| {
        let rx = Receiver::new(buffer_child);
        let tx = Sender::new(SharedRingBuffer::open(&name_parent).unwrap());
        let mut rng = rand::thread_rng();
        let bench_data = BenchData {
            block_hash: (0..100).map(|_| rng.gen_range(0, 254)).collect(),
            chain_hash: (0..64).map(|_| rng.gen_range(0, 254)).collect(),
        };
        while rx.recv::<BenchData>().is_ok() {
            tx.send(&bench_data).unwrap();
        }
    });
    assert!(child_pid > 0);

    let rx = Receiver::new(buffer_parent);
    let tx = Sender::new(SharedRingBuffer::open(&name_child).unwrap());

    let mut rng = rand::thread_rng();
    let bench_data = BenchData {
        block_hash: (0..100).map(|_| rng.gen_range(0, 254)).collect(),
        chain_hash: (0..64).map(|_| rng.gen_range(0, 254)).collect(),
    };
    b.iter(|| {
        for _ in 0..100 {
            tx.send(&bench_data).unwrap();
            let _ = rx.recv::<BenchData>().unwrap();
        }
    });
}

fn bench_uds(b: &mut Bencher) {
    let sock_path = temp_sock();

    let child_pid = fork(|| {
        // wait for parent to be ready
        let sock_path = sock_path.as_path();
        while !sock_path.exists() {
            thread::sleep(Duration::from_millis(20));
        }

        let client: IpcClient<BenchData, BenchData> = IpcClient::new(&sock_path);
        let (mut rx, mut tx) = client.connect().unwrap();
        let mut rng = rand::thread_rng();
        let bench_data = BenchData {
            block_hash: (0..100).map(|_| rng.gen_range(0, 254)).collect(),
            chain_hash: (0..64).map(|_| rng.gen_range(0, 254)).collect(),
        };
        while rx.receive().is_ok() {
            tx.send(&bench_data).unwrap();
        }
    });
    assert!(child_pid > 0);

    let mut server: IpcServer<BenchData, BenchData> = IpcServer::bind_path(&sock_path).unwrap();
    let (mut rx, mut tx) = server.try_accept(Duration::from_secs(3)).unwrap();

    let mut rng = rand::thread_rng();
    let bench_data = BenchData {
        block_hash: (0..100).map(|_| rng.gen_range(0, 254)).collect(),
        chain_hash: (0..64).map(|_| rng.gen_range(0, 254)).collect(),
    };
    b.iter(|| {
        for _ in 0..100 {
            tx.send(&bench_data).unwrap();
            let _ = rx.receive().unwrap();
        }
    });
}

benchmark_group!(benches, bench_shm, bench_uds);
benchmark_main!(benches);
