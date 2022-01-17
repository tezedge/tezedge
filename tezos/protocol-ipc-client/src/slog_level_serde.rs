// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Serialize, Deserialize)]
pub enum SlogLevel {
    Critical,
    Error,
    Warning,
    Info,
    Debug,
    Trace,
}

impl From<SlogLevel> for slog::Level {
    fn from(v: SlogLevel) -> Self {
        match v {
            SlogLevel::Critical => Self::Critical,
            SlogLevel::Error => Self::Error,
            SlogLevel::Warning => Self::Warning,
            SlogLevel::Info => Self::Info,
            SlogLevel::Debug => Self::Debug,
            SlogLevel::Trace => Self::Trace,
        }
    }
}

impl From<slog::Level> for SlogLevel {
    fn from(v: slog::Level) -> Self {
        use slog::Level::*;
        match v {
            Critical => Self::Critical,
            Error => Self::Error,
            Warning => Self::Warning,
            Info => Self::Info,
            Debug => Self::Debug,
            Trace => Self::Trace,
        }
    }
}

pub fn serialize<S>(value: &slog::Level, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    SlogLevel::from(*value).serialize(s)
}

pub fn deserialize<'de, D>(d: D) -> Result<slog::Level, D::Error>
where
    D: Deserializer<'de>,
{
    SlogLevel::deserialize(d).map(|x| x.into())
}
