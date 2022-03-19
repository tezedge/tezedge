// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp;

use slog::{error, info, Logger};

const OPEN_FILES_LIMIT: u64 = 64 * 1024; //64k open files limit for process

// Enables and sets maximum size for core dump file if it was not set before
// If user set a non-zero limit before, it will be left as is
unsafe fn enable_core_dumps(log: &Logger) {
    // Get current size limit of core dumps
    let rlim = match rlimit::getrlimit(rlimit::Resource::CORE) {
        Ok(rlim) => rlim,
        Err(e) => {
            error!(log, "Enabling core dumps failed (getrlimit): {}", e);
            return;
        }
    };
    let (mut soft, hard) = rlim;
    // Bump soft limit to the hard limit, but only if it was not set before
    if soft.as_raw() == 0 {
        soft = hard;
        if soft.as_raw() > 0 {
            match rlimit::setrlimit(rlimit::Resource::CORE, soft, hard) {
                Ok(()) => info!(log, "Core dumps enabled with maximum size."),
                Err(e) => {
                    error!(log, "Enabling core dumps failed (setrlimit): {}", e);
                }
            }
        }
    }
}

// Sets the limit of open file descriptors for the process
// If user set a higher limit before, it will be left as is
unsafe fn set_file_desc_limit(log: &Logger, num: u64) {
    let num = rlimit::Rlim::from_raw(num);

    // Get current open file desc limit
    let rlim = match rlimit::getrlimit(rlimit::Resource::NOFILE) {
        Ok(rlim) => rlim,
        Err(e) => {
            error!(log, "Setting open files limit failed (getrlimit): {}", e);
            return;
        }
    };
    let (mut soft, hard) = rlim;
    // If the currently set rlimit is higher, do not change it
    if soft >= num {
        return;
    }
    // Set rlimit to num, but not higher than hard limit
    soft = cmp::min(num, hard);
    match rlimit::setrlimit(rlimit::Resource::NOFILE, soft, hard) {
        Ok(()) => info!(log, "Open files limit set to {}.", soft),
        Err(e) => error!(log, "Setting open files limit failed (setrlimit): {}", e),
    }
}

pub fn init_limits(log: &Logger) {
    // Need unsafe to access direct libc syscall interface
    unsafe {
        // Enable core dumps for debug build
        if cfg!(debug_assertions) {
            enable_core_dumps(log);
        }
        // Increase open files rlimit to 64k
        set_file_desc_limit(log, OPEN_FILES_LIMIT);
    }
}
