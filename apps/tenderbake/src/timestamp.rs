// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::{
    fmt,
    time::Duration,
    ops::{Add, Sub},
};

/// Timestamp as a unix time
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Timestamp {
    pub unix_epoch: Duration,
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let int = self.unix_epoch.as_secs() % 3600;
        let fr = self.unix_epoch.subsec_millis();
        let minutes = int / 60;
        let seconds = int % 60;
        write!(f, "{:02}:{:02}.{:03}", minutes, seconds, fr)
    }
}

impl Add<Duration> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: Duration) -> Self::Output {
        Timestamp {
            unix_epoch: self.unix_epoch + rhs,
        }
    }
}

impl Sub<Timestamp> for Timestamp {
    type Output = Duration;

    fn sub(self, rhs: Timestamp) -> Self::Output {
        self.unix_epoch - rhs.unix_epoch
    }
}

impl Sub<Duration> for Timestamp {
    type Output = Timestamp;

    fn sub(self, rhs: Duration) -> Self::Output {
        Timestamp {
            unix_epoch: self.unix_epoch - rhs,
        }
    }
}
