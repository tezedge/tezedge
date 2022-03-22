// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::panic::UnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;

use slog::{Drain, Duplicate, Level, Logger, Never, SendSyncRefUnwindSafeDrain};

use crate::detailed_json;
use crate::file::FileAppenderBuilder;

#[macro_export]
macro_rules! create_terminal_logger {
    ($log_format:expr) => {{
        match $log_format {
            LogFormat::Simple => slog_async::Async::new(
                slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
                    .build()
                    .fuse(),
            )
            .chan_size(32768)
            .overflow_strategy(slog_async::OverflowStrategy::Block)
            .build(),
            LogFormat::Json => {
                slog_async::Async::new(detailed_json::default(std::io::stdout()).fuse())
                    .chan_size(32768)
                    .overflow_strategy(slog_async::OverflowStrategy::Block)
                    .build()
            }
        }
    }};
}

#[macro_export]
macro_rules! create_file_logger {
    ($log_format:expr, $file_cfg:expr) => {{
        let appender = FileAppenderBuilder::new($file_cfg.file.clone())
            .rotate_size($file_cfg.rotate_log_if_size_in_bytes)
            .rotate_keep($file_cfg.keep_number_of_rotated_files)
            .rotate_compress(true)
            .build();

        match $log_format {
            LogFormat::Simple => slog_async::Async::new(
                slog_term::FullFormat::new(slog_term::PlainDecorator::new(appender))
                    .build()
                    .fuse(),
            )
            .thread_name("slog-background-thread".to_string())
            .chan_size(32768)
            .overflow_strategy(slog_async::OverflowStrategy::Block)
            .build(),
            LogFormat::Json => slog_async::Async::new(detailed_json::default(appender).fuse())
                .thread_name("slog-background-thread".to_string())
                .chan_size(32768)
                .overflow_strategy(slog_async::OverflowStrategy::Block)
                .build(),
        }
    }};
}

#[derive(Debug, Clone)]
pub struct SlogConfig {
    pub log: Vec<LoggerType>,
    pub level: Level,
    pub format: LogFormat,
}

impl SlogConfig {
    pub fn create_logger(&self) -> Result<Logger, NoDrainError> {
        let drains: Vec<Arc<slog_async::Async>> = self
            .log
            .iter()
            .map(|log_target| match log_target {
                LoggerType::TerminalLogger => Arc::new(create_terminal_logger!(self.format)),
                LoggerType::FileLogger(file_cfg) => {
                    Arc::new(create_file_logger!(self.format, file_cfg))
                }
            })
            .collect();

        if drains.is_empty() {
            Err(NoDrainError)
        } else if drains.len() == 1 {
            // if there is only one drain, return the logger
            Ok(Logger::root(
                drains[0].clone().filter_level(self.level).fuse(),
                slog::o!(),
            ))
        } else {
            // combine 2 or more drains into Duplicates

            // need an initial value for fold, create it from the first two drains in the vector
            let initial_value =
                Box::new(Duplicate::new(drains[0].clone(), drains[1].clone()).fuse());

            // collect the leftover drains and fold the drains into one Duplicate struct
            let merged_drains: Box<
                dyn SendSyncRefUnwindSafeDrain<Ok = (), Err = Never> + UnwindSafe,
            > = drains.into_iter().skip(2).fold(initial_value, |acc, new| {
                Box::new(Duplicate::new(Arc::new(acc), new).fuse())
            });

            Ok(Logger::root(
                merged_drains.filter_level(self.level).fuse(),
                slog::o!(),
            ))
        }
    }
}

#[derive(Debug, Clone)]
pub struct NoDrainError;

#[derive(Debug, Clone)]
pub enum LogFormat {
    Json,
    Simple,
}

impl std::str::FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "simple" => Ok(LogFormat::Simple),
            "json" => Ok(LogFormat::Json),
            _ => Err(format!("Unsupported variant: {}", s)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum LoggerType {
    TerminalLogger,
    FileLogger(FileLoggerConfig),
}

impl LoggerType {
    pub fn possible_values() -> Vec<&'static str> {
        vec!["terminal", "file"]
    }
}

#[derive(Debug, Clone)]
pub struct FileLoggerConfig {
    file: PathBuf,
    rotate_log_if_size_in_bytes: u64,
    keep_number_of_rotated_files: u16,
}

impl FileLoggerConfig {
    pub fn new(
        file: PathBuf,
        rotate_log_if_size_in_bytes: u64,
        keep_number_of_rotated_files: u16,
    ) -> Self {
        Self {
            file,
            keep_number_of_rotated_files,
            rotate_log_if_size_in_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::fs::File;
    use std::path::Path;

    use libflate::gzip::Decoder as GzipDecoder;
    use slog::info;
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn test_logging_without_rotation() {
        // prepare cfg and logger
        let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined");
        let log_dir_path = Path::new(out_dir.as_str()).join("test_logging_without_rotation");
        if log_dir_path.exists() {
            fs::remove_dir_all(&log_dir_path).expect("Failed to delete log directory!");
        }
        fs::create_dir_all(&log_dir_path).expect("Failed to create log directory!");
        let log_file_path = log_dir_path.join("test.log");

        let file_logger_without_rotation = FileLoggerConfig {
            file: log_file_path.clone(),
            keep_number_of_rotated_files: u16::MAX,
            rotate_log_if_size_in_bytes: u64::MAX,
        };
        let slog_config = SlogConfig {
            log: vec![LoggerType::FileLogger(file_logger_without_rotation)],
            level: Level::Info,
            format: LogFormat::Simple,
        };
        let log = slog_config
            .create_logger()
            .expect("failed to create logger");

        // do some logging
        let expected_log_count = do_logging(&log);

        // drop causes a flush on logger
        drop(log);

        // assert and check
        let logs_count = count_lines(&log_file_path);
        assert_eq!(logs_count, expected_log_count);
    }

    #[test]
    fn test_logging_with_rotation() {
        // prepare cfg and logger
        let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined");
        let log_dir_path = Path::new(out_dir.as_str()).join("test_logging_with_rotation");
        if log_dir_path.exists() {
            fs::remove_dir_all(&log_dir_path).expect("Failed to delete log directory!");
        }
        fs::create_dir_all(&log_dir_path).expect("Failed to create log directory!");
        let log_file_path = log_dir_path.join("test.log");

        let file_logger_with_strong_rotation = FileLoggerConfig {
            file: log_file_path,
            keep_number_of_rotated_files: u16::MAX,
            rotate_log_if_size_in_bytes: 512 * 1024, // 0.5 MB
        };
        let slog_config = SlogConfig {
            log: vec![LoggerType::FileLogger(file_logger_with_strong_rotation)],
            level: Level::Info,
            format: LogFormat::Simple,
        };
        let log = slog_config
            .create_logger()
            .expect("failed to create logger");

        // do some logging
        let expected_log_count = do_logging(&log);

        // drop causes a flush on logger
        drop(log);

        // assert and check all rotated files
        let mut logs_count = 0;
        for log in fs::read_dir(&log_dir_path).unwrap() {
            let log = log.unwrap();
            let name = log.path().display().to_string();
            if name.ends_with(".gz") {
                // decode
                let mut decoder =
                    GzipDecoder::new(File::open(log.path()).expect("failed to open log file"))
                        .expect("failed to decode");
                use std::io::Read;
                let mut buf = Vec::new();
                decoder.read_to_end(&mut buf).unwrap();

                use std::io::Write;
                let mut tmp_file = NamedTempFile::new().expect("failed to create temp file");
                tmp_file.as_file().write_all(&buf).expect("failed to write");
                tmp_file.flush().expect("failed to flush");

                // count lines in decoded
                logs_count += count_lines(&tmp_file.path().to_path_buf());
            } else {
                logs_count += count_lines(&log.path());
            }
        }
        assert_eq!(logs_count, expected_log_count);
    }

    fn do_logging(log: &Logger) -> usize {
        let mut expected_log_count = 1;
        info!(log, "Logging tests started...");

        // thread1 logging
        let handle1 = {
            let log = log.clone();
            std::thread::spawn(move || {
                for i in 0..100 {
                    info!(log, "Peer state info";
                               "current_head_update_secs" => 0,
                               "current_head_level" => "1553789",
                               "mempool_operations_response_secs" => "46633",
                               "mempool_operations_request_secs" => "46633",
                               "queued_block_operations" => "-empty-",
                               "queued_block_headers" => "-empty-",
                               "current_head_response_secs" => "0",
                               "current_head_request_secs" => "46633",
                               "actor_ref" => "ActorRef[/user/peer-845]",
                               "counter" => i);
                }
            })
        };
        expected_log_count += 100;

        // thread2 logging
        let handle2 = {
            let log = log.clone();
            std::thread::spawn(move || {
                for i in 0..100 {
                    info!(log, "Peer branch bootstrapper processing info";
                                "blocks_scheduled_for_apply" => "754156",
                                "block_intervals_next_lowest_missing_blocks" => "",
                                "block_intervals_scheduled_for_apply" => "28338",
                                "block_intervals_open" => "0",
                                "block_intervals" => "28338",
                                "peers_branches_level" => "1553787 (BMMX3nRx426q87WwQoF6vCmqc9QXERYCkBC66XNVZhTBBuKqm8Z), 1532332 (BL35kEu2LHqvuBjZgdTKQq2Lwa4bUViZDNAzYnYDqHk1vmC1sbg), 1180237 (BLSKJpy68mkwfy38Aeg5XdLF17aTBjWS5sjAsMWx7ztNP4sFGYK), 1212415 (BKnEEKKYH24nzgsYdLEiJ3gzMeRskwXJLmAxB4R2UQmsckctRMU), 1546452 (BL3FzcowJTe5YKyoG8mJ6dodt9Q9j77q6dNwx2MstXgjqBvnraN), 1540075 (BM9RvBp2X5EsS6kocXRWxLRdPs3mM1VKiR6po7B4SPUHLFiLKYy), 1550840 (BLFgeJP1Ac7KxyfhmEXDyiEw9JZjNSREHqvJhSdBMJ69eoxWyfh), 1548565 (BLysmDdAbrhmjUViyk4wXStiXARvq5L6WBhauHgKeGieYmWWjd6), 1343487 (BLM76Xc4tH3YDEFnF5YYLCYkcRoZSJ6efHZ5YkfwuDnjWpTUHq2), 1248386 (BL9W1AwRdTQmgiWLpruANvUmkrpbbY1E8aRy7hMayZjnWsdmNrq), 1540069 (BMQ8K7fLyzkUF5Azv5xigRxeizr3HQeGHdT3Ei1EjjU4Qsv5NVL), 1553144 (BMazpAPviQm6MhFM7KqWwKkD8YfLQympm4X9eee5HeUBbD5HAuY), 1553788 (BMWQhWdDeY5PPLf63qYbJ6DVmzppT7rsnPH7HA76StUQR3B1P3A), 1552103 (BL9BuhHwQhqUefaTxM5uWoX8xytmmmyQfiw5p68EVCevmo8vezR), 1482040 (BMXgbphScLtqJmQW28ZUCB7ev35bhRgXnHK2MJpYF3V2BhYrFXN), 1525463 (BLYyB5cpLXPYPcpBevpoBPn5SurbrVUYJucpXfWDsGEWjunrkQe), 1466366 (BMHFBR8os4hxdt6Gm2uux1jncHbaGwcSbQx7VwmcxYWgCsEMhwn), 1143778 (BMbhhBRDk89rXA6Yygw4nS6VGS8cQLHpuYHds764zhNJEBDEwtd), 1488269 (BLMzC56FcXwaTCF6UyHrMVLXZbPJxMFyECidJWuo1NxErGxEsX9), 1466461 (BMAJh76n2teXmwEJnNi8gNZSTsDJwMiuaDXkL7cCp48g9ibervw), 1494334 (BMHPKReqvuh7nDd8vEH8QX6CXZXwLhBWhRypequtQvUucYNkm3y), 948160 (BKwngZTVSxCT3orXqjCz2pZeQWW3eg255tk5cVx5wzGEsPZ3B7t), 1540065 (BMGqR1FXjQnm25ns3zbSLykzoJvgymwG14cyJBsYBpXBq438Gce), 1540107 (BKuQX8G2cPwqmtkxgBnN8XqN4HeHz9RX2rbZu9XQWnLCqjJVMMG)",
                                "peers_branches" => "79",
                                "peers_count" => "78",
                                "actor_received_messages_count" => "1604",
                                "counter" => i);
                }
            })
        };
        expected_log_count += 100;

        // thread3 logging
        let handle3 = {
            let log = log.clone();
            std::thread::spawn(move || {
                for i in 0..100 {
                    info!(log, "Branch bootstrapping process was updated with new block";
                                "peer_uri" => "/user/peer-986",
                                "peer" => "peer-986",
                                "peer_ip" => "116.124.142.150:58442",
                                "peer_id" => "idtfbbLKvb252jwTsL1yNm915WH87A",
                                "new_block" => "BMWQhWdDeY5PPLf63qYbJ6DVmzppT7rsnPH7HA76StUQR3B1P3A",
                                "counter" => i);
                }
            })
        };
        expected_log_count += 100;

        // thread4 logging
        let handle4 = {
            let log = log.clone();
            std::thread::spawn(move || {
                for i in 0..100 {
                    info!(log, "Start branch bootstrapping process";
                                "peer_uri" => "/user/peer-1015",
                                "peer" => "peer-1015",
                                "peer_ip" => "116.202.84.154:33540",
                                "peer_id" => "idt5tqk3nFc1pNw1PaxmjheBaHZRgu",
                                "to_level" => "1212415",
                                "missing_history" => "BLzQGEk3Fayc5hvhMNQ23EPqzkmwGRfSEYY6db4RfwDbdTayeZk, BL8FSLFqiCroJLJyWfNijBzMzF1zYrPj3ZEEyb9oeHMJw3S7uxx, BKpU8ZTZyJ6JQAXZaj5HUcjhJW9fyLUowwxFP1ksdsSYFsybDNY, BLXdiTjcStAkB268ojKHDdM7Fd3zA7twrKFZvqAU7Lkct8bsRyj, BLoEKoYzup3Fpo6KgkPaX91PMPg613ru57yp9yUHQWU6GNSuSMe, BKoa9XUYkxi4Pxvb3pwJanx7FwzG4eciYKvWdrTrMWgwQK7kWog, BL5SrGJ85bbPFTookURATZXqCPRYgiVHPdk6bRXn1j696eLJ6Hk, BMFGvWhPU89FYLeMFQ9nGm54491BsVehpo9R6QQumNZPnnoDEmf, BL2YkkE7XWnEBHFHqZ2pHFjZoboCXT6pBqrh6tMSav4Gh35eos1, BLYYSviqrCWSTC15N3mKaEepGY5ERxtACuK9iXJgBhe2H5RLN3M, BMGrNCDdw8mjf5dpVhEJmjcmfsXALz1RkDTQ5eAzn6iuL4V6aEC, BMV9dASyb91nJbmfdKeV9HZrzSLyboaZepM81ziNhvHduNcLHZG, BLrQHMsf93ihSySSu3R2RSNQ1P1zmiRvhX6sSdUggUfDxU3YH74, BLHXwDXBtqvbzzbqgxncxk8LCWsbL89bXYaYq2VYLHrZgjutuRi, BMY7FmmU37fzUtzuBiXAV3Yq7zsTzacKdnVLJZEVojLyZHBMywn, BLtuMjcUM6P3uFN9wWg12BG72tGnCmLti2B9Ppu6LdUxSkRsvxa, BLU4RBNdFUTRW8rGQgRbay36GKJLANJomr1bSTo1bMfvauAYaeH, BLQA8GeEtDi9vZeihrF4qkc9QUrmTMQGXqGbB1hSo7RgLwrxSEY, BLMEp5KWSZM14DdZm4qwCjbh8VjRtLTCrhEkN4EG3NMdsvxkqr8, BLh8qSHNxTB6QsatjX68LYVu5VxNKt7EjA8GiFqxTbKCeNSGQPX, BMaAxAQeVBJL7Xn7rcvL8uwfexpcPKWhCRmKxBtJffZ8rrHzydL, BLxpTLYovNhS9yarmCTAaU7UKushH3QLFqNM4TxvwAVjWEjNEuF, BLjVTYYpWpsECPotm35W4i8XRnRwM2cMjwTkJ47jvtcCD73rqBD, BMbmBQ9nV1cE66owYGev2f8NzqPAbCt5MoaW8mSVWjd78edsydb, BLKKq883ytrn9Gg9t5Wr4cztUeDH3AC8ATdHx7XQvTxWBA2wE8F, BKsisBMGzwiLni1wmkUe8WeqAZz8DxQScwBkS5w9ZjpTg29JhKJ, BMDeuLz4QXFCLdAyLQjmqqFan3joVYXSubUB825WwTQnq8yosGa, BLg27KvzDgFzLADWTv3HEfqr1ayd8VGzx8pLNRZcUYQjJ3wTaCL, BMZHDsxPcqDqYp13wW3sAocqqDRVPiGtzZyGk8hFL9FLU8xJi31, BL7P6LcKnjjQCP9GPwq5d8nRHwbJwTQJnj1wHaF6nJkFEHwECoq, BLAvcTRFdBzi9puuAp18GcrrMkWpaYUHzEVZ9oxJ13LivqA5izo, BL9C2yAkCyXarYVKBPCxAGMpauC8Fav1SvK78HwFYZb3Dgyv8BV, BLNNokDjZt1BpTg2T7cDCawDqExK5FAxLopD9cK8BwxzZFXd8ht, BM8Y3kKzBMUiMty4oeghW6CfUAZ3DBpKSUVs9usFDALKttdUkSx, BMNMjDNMjEEYPcj3V7MdeY6xe6K7S6bF9vidQaxqmUVUJPpqweX, BLadUi1hkz3gMacSmWQnR3S9sjMohfsS5DmzsFoKvGJL7G66hxX, BMP89G1vDMxgWMm3GHVKz8pjKzfxLYjWG69wiL82x7u9it6xtZG, BLN94VGxPEtQGLPvkURphtxGxik4oDSuEP8vg1515RLtvsggDW6, BL7gEZ3zb6fAQD1dmDqopcLKAciRNjdUQk5p4fiE3SRpDozKz4T, BLNYBZz7G2CVHsCYZwTfTt1mG3K6KcfgSRhsVHgSZ7LmwwSNCou, BLBgznyHJ21dbknexSDfbGs2bRMaUprN5FjVGekJqyPKV1m1of6, BMM62dQkP4iskBBREcvHrPReRuqeXztze52F2BdBuGBKrfGwtsR, BM14xK189gfMhjMit9mzxtcTrEhvLrESWxvjVyzSi94PUqWtJ8G, BL9hVjZig1eCbRN8ejMV5sMVP57PGb9cg6w8e2C8xVC9Y1nY61K, BL7QBmG9QPDccYDLPc2WdhLD4efpnApD284WfSDcZXFJF12swyH, BMGkC5EMnxJgAq3M8AZZgGAK7dUYL8JKM3imUx1bHECS4KFJjEH, BMXTaGyG3znUc2LArZ22YLCB6k96M6EbnE27MbhTGxYSFCUAX26, BMKfidLhAU7y1nRHBGctDDqj1gHf77JQmxTobzyPRpxqQYjGUWc, BMJ73PHX6M1LB6CossLyePjpLeEjCk13VqcCU6QKB1a4cAVqiou, BKqA1Qgit9aSzkLJa7n5x11uWuJhvC9Mc3YSXxENVhn2ihcNwVo, BL2LUv3JhFK2Z5GoK5ZzVAnaA46W5zTf9VTmwTK9SsXxsREwica, BLWhc9ULKywohS9WWMSAXiD3jsmw2cqiKkZcuGo2qyaMDEhkViS, BLsxeSMe4wChm4bdN3XpM49gUSpomVJJSXWPJaycQgPVJHWpBC1, BMX2wqPHrHPUUEc7cEJ9kqWhLB4jzsow9LrtqWVNXC1URS3D4pr, BMAqSAhBQw31yhKNTQ49PqMxrR1WLVypfFkziHhB36hzC9ridVg, BLVB24N4SxMeLf5fj72it5dXzbXCzMEBdqSGZR9Adzg1GZDZe5W, BLdrvzeKSLuQeLDw6LhsyhaEJTfhNUfW5ufMmCLL3rsz7LADb6b, BLuNyRrkJZBb4xfqDzrNmdq2GzSgg7gKmch6KvpKthkMZLDLqPC, BMDkbeV6vcRJithNDBwqbFkCmvCXAJFqt5L3c3x8w6zFC2pARQ4, BME1Jw9irqxDeJJipqAo9bhUTpufTdh74oWPqbkBjut2vLwsdRg, BM569TCLpMnXrmN9hy5C3z7sfphRyPef2pPUqfjMpPTFpawrHd1, BLDmfSeJ1A4hA4xb2KFaBxu63apaQoeHyuGqkabkCN5dnNKkKCt, BMNnwMRNrWhutwX2KvEFWFTmYkzMKSKG2edcnmZn8To3kw3Ez6L, BLaDyExNVftqeutkRRR8eaT3tMZawQTHEr1tNf41WzCF3GKekZN, BLsVUK3NxEg2DX5VnsbpHuPnKJWMQHYg87yNsSYRygZQtnYvosi, BL9kUSGsdaXf8grcGTe666ATbsRPTYxFeuXiMHx68ekkXrRtuq2, BKuf4s62dBcZqjeG8uf4gPBcyATidqSPZGfBfA217ddAeDeqd3s, BLRME9BRhBRYVAbgiWYdATNbynDMf5Db1V5aDEFJdihpA61SZwF, BLVYQmUCiduQgUQvyG8duQrcYQS5Fpsi9BAp39YYZroaLUSy9kM, BKyumaivKS1granZQj7NQkFgnB7gjGr4dHNXXJLbPGWfURp5V3t, BLFAxJqMik94JYxYAvWEmiHwRC7HSXi4Rf6b4T1MqM498hNqczF, BMcXUXi3oKBiUiQcvi9KkE68SWWMEwhcsYduqKYdHamhBfyFvJT, BKugqESZFFizPecoU6tNi5SQE67U7n6Y48GJXYgtGPJdcyW5A79, BLfvmsavRVg1mZYRXECzpD8YmAJreMRGoMoLxjaV7RqWQmZ7kAc, BLHCM7dgPmxfMwFk9dN8ScrWGRfYpSMeANiMGfMnp7sPqUrjNVn, BLUURXTYKMp1dPuDjP4nbFkcbRt7gkJaF4q71jWZiJbZJ2uFYKT, BMdjvweHwWiNiZEYk2HiL5pRdBZVThmQ7bTGnzJoNLHwLdwttdn, BKjbaQhsQBTiuemdiVQyAfdRXiY2DxXatV56wbr6v3NKBBK9WMu, BLxjhL3aCjKwfLAf4ZVQGgoLVcdpDjZk4utkCkBdKY7XvecQGE4, BMLrLuVo58T8ECX8JAADqgt9M6hvSjtoeZ4s6rQxJ1Ls2HyD4iJ, BLksTcVsF2P4vpAM37TiVDnLUkX4Nhx4fmKufcjk41qhqM7Vj9Y, BKkEbaFtRoZUF7t1niKh7osrkNvPtYrKprct9MSW5jPw5nLboxr, BLui9hg6dhRjw8rrPBfDud3kMbgKTM3f95v4ba5UVbXHaQAHTB2, BLMidcJtUXi9jn6k2hWujj1NHh6qWnqg2nVvFSywuLd5XDzCUrk, BM3UXpW6iL7E6gDQ2tec47oyUznKwu6Uhqx5i1BxzLGXiedayxS, BM1kr8haKMfKmxcimjKJvBS1X8barPadEW9kAH32ErrCRAT3epK, BLhS2nEBCQ99Sg3w3fVvNWF2HdRQw2SSPvWbSQEvitKK6af9JG7, BLuFYekTY9qqFg9rxCmBvXcm1FbPJmRMrWBDtJWoFgSgGsuHSzu, BLCZ72grJ7RvKKTigr4nvM1tkZQxadD92vnbc7gEdgMer5d1iQu, BLCdSk9n8ozW6EbgsSJj5o2WfsMahKvG6Qyw1Vk7SVB1nHKxJce, BLKS1SeBjvA3fWPFejuKApZkAej1RjiAMs8uFSG7PWTxxdsxKCP, BKwTSDqDMGmKB8KtWqTCHV3sEM5zFSBVU8KtbbmYQmSCcbmCh8H, BKmixYp2MvXGvN1s8Eo68ipeYMvSM7N9tRabpeH8b9zZfVwHC83, BLxafqYDFoMGYc6NRwbGYdbdP7YUf4i2WGmWRCcYKgY7KoVxgGs, BKmqZ8duTKtGsp5KJD9qzt5ertudqn6afHjF52To4kxYza7ybtS, BMUfsJu8yfNwMUnW2UUCvCodb7D8YTAeEBxyJu5e88zZcVJQhdZ, BLhKBUV5oYhyMdPoGLoUVeYcsbZ1rV67TCQVyL1nDxJDs55YnNP, BKjeMoNpu76WgrfUEf7wXA9S2Ssbf83zXstz8XtQepKrUKoRvsn, BMcUHUHprQQo9ruWt1fUsT7RFuVY8SmCQV83S8MgrCgxe4JKpww, BMQf8JA2roy53w4V1GZDvVxteBKVpEQbP7ziHTkW7pCKduZrqo6, BLs98pKC8edDohnedqrj1ms6kWtsG9YxCpRzZwaG9wu9x811ig3, BL3xntq1kwMSPjKYR428Fvanw7y4hs84a5LU9fUXKGwSLgDsMQD, BKqF2ZQpUPfwgYmjy4Cic2SJhrcTpbiiA46ZLVXtUfk9diudqAo, BL9p4hy2922du4FWMrW2E6p4vj7oE1kZyGJQHc1KVWKbWXC5LWi, BLuky56yrNPwfnFxiPTdyWB5NPFTzLbpVQKckvHA6e9bpS9L9Qx, BLfoE5GffkHeSHoq3rseoAfY6tqu11UX4LfDiaFKAP2nY9ZjW3o, BKtwKTzHEGMgr4WMSLLgUneJrUU7SMrzRNuqrK1JE8jMcHv38nz, BLPTRTWatKY9pe9LKA24ZcdaiVj6gGC1c7Gr9Q22xXR4z37UMwa, BLyVKZ1awWZWMCnT9e8aYVCUTQUiCVrL2aKntGBxdyXVodbDdf8, BKtczfM6dYtqExqcnCwn71vFQDZQMMME5ijswdgCq8gnJr7DM5G, BLFdvWU1pC78zvrTMqXZs3cJhLyxPs9HCRBMfajYsXd3U3TAzpX, BMMvgHouFYD3pbSTx8CpLFbA9wQBYNhqqsSV5LnGx82oMDT55j2, BKmb1a3j5JPn9EDJrgScghJwCaK5E6aJXqaG74nxF35GG7R5rps, BLoik7GwNDhUiBTz2d7z1u4ZHiirVoiVTFxZ6gNgkYoPfFgw8ih, BMBdDWVFwYmvdgmEDWmMUyY62j3cgRPY7CFCWQd4bVP9cTc2QWs, BLqp2aRxY3zNdoiv8bFk4SVQgSW1Q2FjHBhYQAvNwLrUMgnutnK, BMEJxsWUGw5sY9JzTYHK5Rh6tuZuo3XCNkNoXVLnSkSq7zvSAix, BL4E9qMNF3q1jZ5HNMSrncZGqNpuWcLzrXWngZ4Y3DzUNA49T37, BKuhkBmYHhq7tJgBzbZPUxwgTnV2eGqoXFNZnCVZQSZTep2orTy, BLziPVDLpeJbePG6nnojGp7AWZiynase3zvfhBWciUZounVcxBJ, BMSVzeWfgCfrPkbqLv7UPzMGpgbWsnBFVQayujFA4yAP9F5R8YU, BM2wPvLFEBVpoAjvLHE2xsfmMY3nTQYmrjt9XLDgber34JwjoeE, BME1LmUg1G6hQZ1niyAAiz4hWCooY64D5BdBRiwE6pkAPLDgsQv, BKqQKpivyaJcycJDhtmaYTy7T6uQAgTTHTLYQ9cM8Z1KfSffxDj, BLtqUrVsv22HF482wQQq6uqjNwkWGj445vhkVYTcbJsXyMs4hJt, BMUsnzuZMAJcYoKwopXMrsrLbWFsaNbuYhkV2FD7KXiiZb4ncvu, BLbpHSkykjJtrt4LWDgVkFZ7uRNsBAqNq5qiDpqTsKPHuG2vnK2, BLmCh8qs4558Fhxo7o7RykhYpQb6Upm7kNsq2Bt63XtqiuxJaPF, BLXJTs2c63f9u3tn6FDxd3SLofv2MwcSG8m7uPHopA9CJoNRtJx, BMSW6DYyfF2EQbqgbSinRyxgHirLy3jf2FSddwy1MdAeYwBLMYb, BMDerYVBgiyHaHfxQKx5irSGWeYkg8CCRM5m89uQz1YcXYvoVUP, BLscPLkyhV4qLFeEpaf4zFhbpXcsSCrr6fSmt2SjuxMuQgELXb8, BMJFz9rXnF4qYiJ26GLpB4jsjFgz8J2DK2epYfuXFebtmFp1TgF, BKkRWcAf71UqbPpvJNY1kvMxqeznQBmZqaDLekVy84xzwvuUUmg, BMKjV4qf6XwX3pmu7LgkAarFMgje8NAxqKRe6KQNdyFwZm74Nk5, BLQzEk1RhdTCdNx4SaBwrfjyN4qqDgpx3zbmBD8CRXJtcpnyb4u, BLK4zqczSDGoT3UEwAmqfQgzgv49G3s8sBsFkcjR8dPRsVk45Tu, BLbJcpGG2yUx8RSQJejwaXhhMpGGUEYhxGefoJxVpv5AdTT6iPB, BMD1XUmFuhsNveiXkqVN6QgMM5s9KF4gwxubKnLvnkBXcqAqjQF, BM3mWzQvQV6mprvVuPqnLtA75XpxX7Bu5ie3ud8Zdfe9XYBsTLH, BM86dXf6HtoZoP6FsxJ5r9dsbtN7w7fUSNhPDqjP6Mtya8vdR2m, BLcJ6TqM52C5twWke6coypKRtSuj7x4NmdCmrhyvKPgd4GX1k3g, BMTMGQm5fQ7y8Pcw5HThiC9pwgYbB9wCLYZebmq6Ba8xmPAtTLz, BLC1kiquLpRS5s2Vji2AjtWCvNeoCboPhLW81tgPP9ZnaxmkVCX, BLL7hU6iymnDhD9kjshX27Kq69RipnxR6aPhLGCc4GDoTUGCmag, BLnoJ56tNEnsBDGmpqCJh739y6SK6UaxQi1R8rrV85g7YRoeZmw, BMdcrMFCbnK8xLQRzsjXHTsNreqibDrfZ2BgnHepX4guUqawEMW, BLFrjfVLpBxP2RBwebBKY1oddWXnhiJGefS4UZmHU7rHHSkaEay, BMGBjwKeqEk2a5eEDrwnjQ955e8d5T3y7m6PeNoMnKggsm18grq, BMZyjUhhoYR4wdxxD5K5xRsiZyMWtGKZrqVvEpZCErCHQmtPPuw, BLRK3CZ6P4dRxvcKj3hGPgAtPX6LNKCWokLvy6Hb7NxkDdJeq4V, BLGf8YHFDab9Z2XetssYQd2WjjirovZq5fPTRN3vpN3zhWT52TX, BKnEEKKYH24nzgsYdLEiJ3gzMeRskwXJLmAxB4R2UQmsckctRMU",
                                "last_applied_block" => "BLXeFVBF3NLaixaZbgV3yJRs9aMDLHFcLdZQeYzYdDvP4espBRC",
                                "counter" => i);
                }
            })
        };
        expected_log_count += 100;

        for i in 0..100 {
            info!(log, "Logging something!"; "paramx" => i);
        }
        expected_log_count += 100;

        handle1.join().unwrap();
        handle2.join().unwrap();
        handle3.join().unwrap();
        handle4.join().unwrap();

        expected_log_count += 1;
        info!(log, "Logging tests finished!"; "expected_log_count" => expected_log_count);

        expected_log_count
    }

    fn count_lines(path: &Path) -> usize {
        use std::io::{prelude::*, BufReader};

        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);
        let mut counter = 0;
        for _ in reader.lines() {
            counter += 1;
        }
        counter
    }
}
