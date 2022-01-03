// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::io::Write;
use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

pub fn print(is_stdout: bool, prefix: &str, s: Option<String>) {
    let bufwtr = if is_stdout {
        BufferWriter::stdout(ColorChoice::Always)
    } else {
        BufferWriter::stderr(ColorChoice::Always)
    };

    let mut buffer = bufwtr.buffer();
    buffer
        .set_color(ColorSpec::new().set_fg(Some(Color::Green)))
        .unwrap();
    buffer.write_all(prefix.as_bytes()).unwrap();
    buffer.set_color(ColorSpec::new().set_fg(None)).unwrap();

    if let Some(s) = s.as_ref() {
        buffer.write_all(s.as_bytes()).unwrap();
    };

    buffer.write_all("\n".as_bytes()).unwrap();

    bufwtr.print(&buffer).unwrap();
}

/// Print logs on stdout with the prefix `[tezedge.tool]`
macro_rules! log {
    () => (print(true, "[tezedge.tool]", None));
    ($($arg:tt)*) => ({
        print(true, "[tezedge.tool] ", Some(format!("{}", format_args!($($arg)*))))
    })
}

/// Print logs on stdout with the prefix `[tezedge.tool]`
macro_rules! elog {
    () => (print(false, "[tezedge.tool]", None));
    ($($arg:tt)*) => ({
        print(false, "[tezedge.tool] ", Some(format!("{}", format_args!($($arg)*))))
    })
}
