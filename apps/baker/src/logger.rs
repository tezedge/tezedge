// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::{o, Discard, Drain as _, Level, Logger};
use slog_async::{Async, OverflowStrategy};
use slog_term::{FullFormat, TermDecorator};

pub fn logger(empty: bool, level: Level) -> Logger {
    use std::io::{self, Write};

    use slog::Record;
    use slog_term::{CountingWriter, RecordDecorator, ThreadSafeTimestampFn};

    pub fn print_msg_header(
        fn_timestamp: &dyn ThreadSafeTimestampFn<Output = io::Result<()>>,
        mut rd: &mut dyn RecordDecorator,
        record: &Record,
        use_file_location: bool,
    ) -> io::Result<bool> {
        let _ = (fn_timestamp, record, use_file_location);
        rd.start_msg()?;
        let mut count_rd = CountingWriter::new(&mut rd);
        write!(count_rd, "{}", record.msg())?;
        Ok(count_rd.count() != 0)
    }

    if !empty {
        let drain = FullFormat::new(TermDecorator::new().build())
            .use_custom_header_print(print_msg_header)
            .build()
            .fuse();
        let drain = Async::new(drain)
            .chan_size(32768)
            .overflow_strategy(OverflowStrategy::Block)
            .build()
            .filter_level(level)
            .fuse();
        Logger::root(drain, o!())
    } else {
        Logger::root(Discard, o!())
    }
}
