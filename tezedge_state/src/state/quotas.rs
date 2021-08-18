// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::Logger;
use tezos_messages::p2p::encoding::peer::{PeerMessage, MESSAGE_TYPE_COUNT, MESSAGE_TYPE_TEXTS};

const THROTTLING_QUOTA_RESET_MS_DEFAULT: u64 = 5000; // 5 secs

lazy_static::lazy_static! {
    static ref THROTTLING_QUOTA_DISABLE: bool = {
        match std::env::var("THROTTLING_QUOTA_DISABLE") {
            Ok(v) => v.parse::<bool>().unwrap_or(false),
            _ => false,
        }
    };

    /// Quota reset period, in ms
    pub(crate) static ref THROTTLING_QUOTA_RESET_MS: u64 = {
        match std::env::var("THROTTLING_QUOTA_RESET_MS") {
            Ok(v) => v.parse().unwrap_or(THROTTLING_QUOTA_RESET_MS_DEFAULT),
            _ => THROTTLING_QUOTA_RESET_MS_DEFAULT,
        }
    };

    /// Quota for tx/rx messages per [THROTTLING_QUOTA_RESET_MS]
    pub(crate) static ref THROTTLING_QUOTA_MAX: [(isize, isize); MESSAGE_TYPE_COUNT] = {
        let mut default = [
            (1, 1), // Disconnect
            (1, 1), // Advertise
            (10, 10), // SwapRequest
            (10, 10), // SwapAck
            (1, 1), // Bootstrap
            (10, 500), // GetCurrentBranch
            (500, 10), // CurrentBranch
            (10, 10), // Deactivate
            (10, 10), // GetCurrentHead
            (500, 500), // CurrentHead
            (5000, 5000), // GetBlockHeaders
            (5000, 5000), // BlockHeader
            (500, 5000), // GetOperations
            (20000, 10000), // Operation
            (10, 10), // GetProtocols
            (10, 10), // Protocol
            (5000, 5000), // GetOperationsForBlocks
            (10000, 10000), // OperationsForBlocks
        ];
        for (i, s) in MESSAGE_TYPE_TEXTS.iter().enumerate() {
            let var = "THROTTLING_QUOTA_".to_owned() + &s.to_uppercase();
            if let Ok(val) = std::env::var(var).or_else(|_| std::env::var("THROTTLING_QUOTA_MAX")) {
                let q = val.split(",").collect::<Vec<_>>();
                if q.len() == 2 {
                    if let (Ok(tx), Ok(rx)) = (q[0].parse::<isize>(), q[1].parse::<isize>()) {
                        default[i] = (tx, rx);
                    }
                }
            }
        }
        default
    };
}

fn decrease(q: &mut isize) {
    *q = q.checked_sub(1).unwrap_or(*q)
}

/// Throttle quotas for PeerMessage.
#[derive(Debug, Clone)]
pub struct ThrottleQuota {
    quotas: [(isize, isize); MESSAGE_TYPE_COUNT],
    quota_disabled: bool,
    log: Logger,
}

impl ThrottleQuota {
    pub fn new(log: Logger) -> Self {
        Self {
            quotas: THROTTLING_QUOTA_MAX.clone(),
            quota_disabled: *THROTTLING_QUOTA_DISABLE,
            log,
        }
    }

    pub fn can_send(&mut self, msg: &PeerMessage) -> bool {
        let index = msg.index();
        if THROTTLING_QUOTA_MAX[index].0 <= 0 {
            return true;
        }
        decrease(&mut self.quotas[index].0);
        if self.quota_disabled || self.quotas[index].0 >= 0 {
            true
        } else {
            if self.quotas[index].0 == -1 {
                slog::warn!(self.log, "Cannot send message because its send quota is exceeded";
                      "msg_kind" => msg.get_type_str());
            }
            false
        }
    }

    pub fn can_receive(&mut self, msg: &PeerMessage) -> bool {
        let index = msg.index();
        if THROTTLING_QUOTA_MAX[index].1 <= 0 {
            return true;
        }
        decrease(&mut self.quotas[index].1);
        if self.quota_disabled || self.quotas[index].1 >= 0 {
            true
        } else {
            if self.quotas[index].1 == -1 {
                slog::warn!(self.log, "Cannot receive message because its receive quota is exceeded";
                      "msg_kind" => msg.get_type_str());
            }
            false
        }
    }

    pub fn iter<'a>(&'a self) -> impl 'a + Iterator<Item = (usize, (isize, isize))> {
        self.quotas.iter().cloned().enumerate()
    }

    pub fn reset_all(&mut self) {
        for index in 0..MESSAGE_TYPE_COUNT {
            let (tx_max, rx_max) = THROTTLING_QUOTA_MAX[index];
            let (tx, rx) = self.quotas[index];
            if tx < 0 {
                slog::warn!(
                    self.log,
                    "Tx quota is exceeded";
                    "msg_kind" => PeerMessage::type_str_for_message_index(index),
                    "quota" => tx_max,
                    "amount" => tx_max.checked_sub(tx).map(|i| i.to_string()).unwrap_or(format!("> {}", isize::max_value()))
                );
            }
            if rx < 0 {
                slog::warn!(
                    self.log,
                    "Rx quota is exceeded";
                    "msg_kind" => PeerMessage::type_str_for_message_index(index),
                    "quota" => rx_max,
                    "amount" => rx_max.checked_sub(rx).map(|i| i.to_string()).unwrap_or(format!("> {}", isize::max_value()))
                );
            }
            self.quotas[index] = (tx_max, rx_max);
        }
    }
}
