// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::process::Command;

use failure::Fail;
use regex::Regex;
use serde::Serialize;

use lazy_static::lazy_static;

pub type MemoryStatsResult<T> = std::result::Result<T, MemoryStatsError>;

#[derive(Serialize, PartialEq, Clone, Debug)]
pub struct LinuxData {
    page_size: usize, // unit of memory assignment/addressing used by the Linux kernel
    size: String,     // total program size
    resident: String, // resident set size
    shared: String,   // shared pages (i.e., backed by a file)
    text: String,     // text (code)
    lib: String,      // library (unused in Linux 2.6)
    data: String,     // data + stack
    dt: String,       // dirty pages (unused in Linux 2.6)
}

#[derive(Serialize, PartialEq, Clone, Debug)]
pub struct DarwinOsData {
    page_size: usize, // unit of memory assignment/addressing used by the Linux kernel
    mem: f64,         // percentage of real memory being used by the process in KB
    resident: String, // resident set size
}

#[derive(Serialize, PartialEq, Clone, Debug)]
#[serde(untagged)]
pub enum MemoryData {
    Linux(LinuxData),
    DarwinOs(DarwinOsData),
}

impl From<LinuxData> for MemoryData {
    fn from(data: LinuxData) -> Self {
        MemoryData::Linux(data)
    }
}

impl From<DarwinOsData> for MemoryData {
    fn from(data: DarwinOsData) -> Self {
        MemoryData::DarwinOs(data)
    }
}

#[derive(PartialEq, Debug, Fail)]
pub enum MemoryStatsError {
    #[fail(display = "IOError: {}", _0)]
    IOError(String),
    #[fail(display = "fail to parse data")]
    ParsingData,
    #[fail(display = "not supported OS")]
    NotSupportedOs,
}

lazy_static! {
    static ref LINUX_DATA: Regex = Regex::new(r"^(?P<size>\d+)\s+(?P<resident>\d+)\s+(?P<shared>\d+)\s+(?P<text>\d+)\s+(?P<lib>\d+)\s+(?P<data>\d+)\s+(?P<dt>\d+).*").expect("Invalid regex");
    static ref MAC_DATA: Regex = Regex::new(r"(?P<mem>[0-9.]+)\s+(?P<resident>\d+)").expect("Invalid regex");
}

pub struct Memory {
    pub pid: i32,
    pub page_size: usize,
}

impl Memory {
    pub fn new() -> Self {
        Memory {
            pid: nix::unistd::getpid().as_raw(),
            page_size: page_size::get(),
        }
    }

    pub fn get_memory_stats(&self) -> MemoryStatsResult<MemoryData> {
        if cfg!(target_os = "linux") {
            self.get_linux_data(format!("/proc/{}/statm", self.pid))
        } else if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
            self.get_mac_data()
        } else {
            Err(MemoryStatsError::NotSupportedOs)
        }
    }

    /// private helpers

    fn get_mac_data(&self) -> MemoryStatsResult<MemoryData> {
        let output = Command::new("ps")
            .args(&["-o", "%mem,rss", "-p", &self.pid.to_string()])
            .output()
            .expect("failed to execute sh command");
        let out_str = String::from_utf8(output.stdout).unwrap();
        self.parse_mac_data(out_str)
    }

    fn parse_mac_data(&self, out: String) -> MemoryStatsResult<MemoryData> {
        if let Some(captures) = MAC_DATA.captures(&out) {
            let data = DarwinOsData {
                page_size: self.page_size,
                mem: captures["mem"].parse().unwrap(),
                resident: captures["resident"].to_string(),
            };
            Ok(MemoryData::from(data))
        } else {
            Err(MemoryStatsError::ParsingData)
        }
    }

    /// get LinuxData from statm String
    /// all exept page_size must be strings convertable to i64
    /// no need to check them because regular will match only numbers /(\d+)/
    fn parse_linux_statm(&self, statm: String) -> MemoryStatsResult<MemoryData> {
        if let Some(captures) = LINUX_DATA.captures(&statm) {
            let data = LinuxData {
                page_size: self.page_size,
                size: captures["size"].to_string(),
                resident: captures["resident"].to_string(),
                shared: captures["shared"].to_string(),
                text: captures["text"].to_string(),
                lib: captures["lib"].to_string(),
                data: captures["data"].to_string(),
                dt: captures["dt"].to_string(),
            };
            Ok(MemoryData::from(data))
        } else {
            Err(MemoryStatsError::ParsingData)
        }
    }

    /// read lines of given file path and parse linux memory data
    fn get_linux_data(&self, filepath: String) -> MemoryStatsResult<MemoryData> {
        let path = Path::new(&filepath);
        let display = path.display();

        let mut file = match File::open(&path) {
            // `io::Error`
            Err(e) => {
                return Err(MemoryStatsError::IOError(format!(
                    "couldn't open {}: {}",
                    display, e
                )))
            }
            Ok(file) => file,
        };

        let mut s = String::new();
        match file.read_to_string(&mut s) {
            // `io::Error`
            Err(e) => Err(MemoryStatsError::IOError(format!(
                "couldn't read {}: {}",
                display, e
            ))),
            Ok(_) => self.parse_linux_statm(s),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn correct_parsing_linux() {
        let statm_to_parse = "218428 10272 6459 7780 0 22424 0".to_string();
        let memory = Memory::new();
        let parse_result = MemoryData::Linux(LinuxData {
            page_size: memory.page_size,
            size: "218428".to_string(),
            resident: "10272".to_string(),
            shared: "6459".to_string(),
            text: "7780".to_string(),
            lib: "0".to_string(),
            data: "22424".to_string(),
            dt: "0".to_string(),
        });
        assert_eq!(Ok(parse_result), memory.parse_linux_statm(statm_to_parse))
    }

    #[test]
    fn correct_parsing_mac() {
        let to_parse = "PID %MEM   RSS\n0.3  5336\n".to_string();
        let memory = Memory::new();
        let parse_result = MemoryData::DarwinOs(DarwinOsData {
            page_size: memory.page_size,
            mem: 0.3,
            resident: "5336".to_string(),
        });
        assert_eq!(Ok(parse_result), memory.parse_mac_data(to_parse))
    }
}
