// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Duration;

use tikv_jemalloc_ctl::{background_thread, epoch, stats, Access, AsName, Name};

#[allow(dead_code)]
#[derive(Debug)]
struct JeMallocStatistics {
    active: usize,
    allocated: usize,
    mapped: usize,
    metadata: usize,
    resident: usize,
    retained: usize,
    background_thread: bool,
    num_runs: u64,
    num_threads: u64,
    run_interval: Duration,
    narenas: u32,
}

fn debug_jemalloc_impl() -> tikv_jemalloc_ctl::Result<JeMallocStatistics> {
    let e = epoch::mib()?;

    let bg = background_thread::mib()?;
    let active = stats::active::mib()?;
    let allocated = stats::allocated::mib()?;
    let mapped = stats::mapped::mib()?;
    let metadata = stats::metadata::mib()?;
    let resident = stats::resident::mib()?;
    let retained = stats::retained::mib()?;

    e.advance()?;

    let num_runs = b"stats.background_thread.num_runs\0";
    let num_runs: &Name = num_runs.name();
    let num_runs: u64 = num_runs.read()?;

    let num_threads = b"stats.background_thread.num_threads\0";
    let num_threads: &Name = num_threads.name();
    let num_threads: u64 = num_threads.read()?;

    let run_interval = b"stats.background_thread.run_interval\0";
    let run_interval: &Name = run_interval.name();
    let run_interval: u64 = run_interval.read()?;
    let run_interval = Duration::from_nanos(run_interval);

    let narenas = b"arenas.narenas\0";
    let narenas: &Name = narenas.name();
    let narenas: u32 = narenas.read()?;

    Ok(JeMallocStatistics {
        active: active.read()?,
        allocated: allocated.read()?,
        mapped: mapped.read()?,
        metadata: metadata.read()?,
        resident: resident.read()?,
        retained: retained.read()?,
        background_thread: bg.read()?,
        num_runs,
        run_interval,
        num_threads,
        narenas,
    })
}

#[cfg(not(target_env = "msvc"))]
pub fn debug_jemalloc() {
    let stats = match debug_jemalloc_impl() {
        Ok(stats) => stats,
        Err(e) => {
            elog!("Failed to get jemalloc statistics: {:?}", e);
            return;
        }
    };

    log!("{:?}", stats)
}

#[cfg(target_env = "msvc")]
pub fn debug_jemalloc() {}
