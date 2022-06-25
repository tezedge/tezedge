// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fs::File;

use slog::{o, Drain as _, Level, Logger};
use slog_async::{Async, OverflowStrategy};
use slog_term::{FullFormat, PlainDecorator, TermDecorator};

pub fn file_logger(file: File) -> Logger {
    let drain = FullFormat::new(PlainDecorator::new(file)).build().fuse();
    let drain = Async::new(drain)
        .chan_size(32768)
        .overflow_strategy(OverflowStrategy::Block)
        .build()
        .filter_level(Level::Info)
        .fuse();
    Logger::root(drain, o!())
}

pub fn main_logger() -> Logger {
    let drain = FullFormat::new(TermDecorator::new().build()).build().fuse();
    let drain = Async::new(drain)
        .chan_size(32768)
        .overflow_strategy(OverflowStrategy::Block)
        .build()
        .filter_level(Level::Info)
        .fuse();
    Logger::root(drain, o!())
}
