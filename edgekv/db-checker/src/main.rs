// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use clap::{App, Arg};
use edgekv::datastore::DataStore;
use std::time::{Duration, Instant};

fn main() {
    let matches = App::new("Database CRC checker")
        .arg(
            Arg::with_name("DATABASE")
                .short("p")
                .long("path")
                .help("Database directory")
                .required(true)
                .takes_value(true),
        )
        .get_matches();

    let database_url = matches
        .value_of("DATABASE")
        .expect("Provide database directory");
    let datastore = DataStore::open(database_url).unwrap();

    println!("Checking {} Keys", datastore.keys_dir().size());
    let instant = Instant::now();
    let mut total_duration = Duration::new(0, 0);
    let mut read_count: u128 = 0;
    for key in datastore.keys() {
        let read_time = Instant::now();
        let _ = datastore.get(key.as_ref()).unwrap();
        total_duration += read_time.elapsed();
        read_count += 1;
    }
    let duration = instant.elapsed();
    println!(
        "Read [{}] items, duration {} secs ~ {} ms",
        read_count,
        duration.as_secs(),
        duration.as_millis()
    );
    println!("Reads per sec {}", read_count / duration.as_secs() as u128);
    println!(
        "Avg read {} μs",
        total_duration.as_micros() / read_count as u128
    );
}
