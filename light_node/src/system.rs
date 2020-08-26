// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use rlimit;

use slog::{info, error, Logger};

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
    if soft == 0 {
        soft = hard;
        if soft > 0 {
            match rlimit::setrlimit(rlimit::Resource::CORE, soft, hard) {
                Ok(()) => info!(log, "Core dumps enabled with maximum size."),
                Err(e) => {
                    error!(log,"Enabling core dumps failed (setrlimit): {}", e);
                    return;
                }
            }
        }
    }
}

pub fn init_limits(log: &Logger) {
    // Need unsafe to access direct libc syscall interface
    unsafe {
        // Enable core dumps for debug build
        if cfg!(debug_assertions) {
            enable_core_dumps(&log);
        }
    }
}
