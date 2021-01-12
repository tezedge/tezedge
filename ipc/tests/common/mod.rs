// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::io::Error;

pub fn fork<F: FnOnce()>(child_func: F) -> libc::pid_t {
    unsafe {
        match libc::fork() {
            -1 => panic!("fork failed: {}", Error::last_os_error()),
            0 => {
                child_func();
                libc::exit(0);
            }
            pid => pid,
        }
    }
}
