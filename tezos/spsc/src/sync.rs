// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#[cfg(not(loom))]
pub(crate) use std::sync::{
    atomic::{
        AtomicUsize,
        Ordering::{Acquire, Relaxed, Release},
    },
    Arc,
};

// `Queue<T::push_slice` uses `std::cell::UnsafeCell` in a way that cannot be
// expressed with `loom::cell::UnsafeCell`
pub(crate) use std::cell::UnsafeCell;

#[cfg(loom)]
pub(crate) use loom::sync::{
    atomic::{
        AtomicUsize,
        Ordering::{Acquire, Relaxed, Release},
    },
    Arc,
};

// Run loom tests with
// LOOM_MAX_PREEMPTIONS=2 RUSTFLAGS="--cfg loom" cargo test --release -- --nocapture
