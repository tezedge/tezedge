use slog::Logger;
use tezos_messages::p2p::encoding::peer::PeerMessage;

const THROTTLING_QUOTA_NUM: usize = 18;

const THROTTLING_QUOTA_STRS: [&str; THROTTLING_QUOTA_NUM] = [
    "Disconnect",
    "Advertise",
    "SwapRequest",
    "SwapAck",
    "Bootstrap",
    "GetCurrentBranch",
    "CurrentBranch",
    "Deactivate",
    "GetCurrentHead",
    "CurrentHead",
    "GetBlockHeaders",
    "BlockHeader",
    "GetOperations",
    "Operation",
    "GetProtocols",
    "Protocol",
    "GetOperationsForBlocks",
    "OperationsForBlocks",
];

const THROTTLING_QUOTA_RESET_MS_DEFAULT: u64 = 5000; // 5 secs

lazy_static::lazy_static! {
    static ref THROTTLING_QUOTA_DISABLE: bool = {
        match std::env::var("THROTTLING_QUOTA_DISABLE") {
            Ok(v) => v.parse::<bool>().unwrap_or(false),
            _ => false,
        }
    };

    /// Quota reset period, in ms
    static ref THROTTLING_QUOTA_RESET_MS: u64 = {
        match std::env::var("THROTTLING_QUOTA_RESET_MS") {
            Ok(v) => v.parse().unwrap_or(THROTTLING_QUOTA_RESET_MS_DEFAULT),
            _ => THROTTLING_QUOTA_RESET_MS_DEFAULT,
        }
    };

    /// Quota for tx/rx messages per [THROTTLING_QUOTA_RESET_MS]
    static ref THROTTLING_QUOTA_MAX: [(isize, isize); THROTTLING_QUOTA_NUM] = {
        let mut default = [
            (1, 1), // Disconnect
            (1, 1), // Advertise
            (10, 10), // SwapRequest
            (10, 10), // SwapAck
            (1, 1), // Bootstrap
            (10, 500), // GetCurrentBranch
            (200, 10), // CurrentBranch
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
        for (i, s) in THROTTLING_QUOTA_STRS.iter().enumerate() {
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

#[derive(Debug, Clone)]
pub struct ThrottleQuota {
    quotas: [(isize, isize); THROTTLING_QUOTA_NUM],
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

    fn msg_index(msg: &PeerMessage) -> usize {
        debug_assert!(THROTTLING_QUOTA_NUM == 18);
        match msg {
            PeerMessage::Disconnect => 0,
            PeerMessage::Advertise(_) => 1,
            PeerMessage::SwapRequest(_) => 2,
            PeerMessage::SwapAck(_) => 3,
            PeerMessage::Bootstrap => 4,
            PeerMessage::GetCurrentBranch(_) => 5,
            PeerMessage::CurrentBranch(_) => 6,
            PeerMessage::Deactivate(_) => 7,
            PeerMessage::GetCurrentHead(_) => 8,
            PeerMessage::CurrentHead(_) => 9,
            PeerMessage::GetBlockHeaders(_) => 10,
            PeerMessage::BlockHeader(_) => 11,
            PeerMessage::GetOperations(_) => 12,
            PeerMessage::Operation(_) => 13,
            PeerMessage::GetProtocols(_) => 14,
            PeerMessage::Protocol(_) => 15,
            PeerMessage::GetOperationsForBlocks(_) => 16,
            PeerMessage::OperationsForBlocks(_) => 17,
        }
    }

    fn index_to_str(index: usize) -> &'static str {
        if index < THROTTLING_QUOTA_NUM {
            THROTTLING_QUOTA_STRS[index]
        } else {
            "<invalid index>"
        }
    }

    pub fn can_send(&mut self, msg: &PeerMessage) -> Result<isize, isize> {
        let index = Self::msg_index(msg);
        if THROTTLING_QUOTA_MAX[index].0 <= 0 {
            return Ok(THROTTLING_QUOTA_MAX[index].0);
        }
        self.quotas[index].0 = self.quotas[index]
            .0
            .checked_sub(1)
            .ok_or(self.quotas[index].0)?;
        if self.quotas[index].0 >= 0 {
            self.quotas[index].1 = THROTTLING_QUOTA_MAX[index].1.min(self.quotas[index].1 + 1);
            Ok(self.quotas[index].0)
        } else if self.quota_disabled {
            Ok(self.quotas[index].0)
        } else {
            Err(self.quotas[index].0)
        }
    }

    pub fn can_receive(&mut self, msg: &PeerMessage) -> Result<isize, isize> {
        let index = Self::msg_index(msg);
        if THROTTLING_QUOTA_MAX[index].1 <= 0 {
            return Ok(THROTTLING_QUOTA_MAX[index].1);
        }
        self.quotas[index].1 = self.quotas[index]
            .0
            .checked_sub(1)
            .ok_or(self.quotas[index].1)?;
        if self.quotas[index].1 >= 0 {
            self.quotas[index].0 = THROTTLING_QUOTA_MAX[index].0.min(self.quotas[index].0 + 1);
            Ok(self.quotas[index].1)
        } else if self.quota_disabled {
            Ok(self.quotas[index].1)
        } else {
            Err(self.quotas[index].1)
        }
    }

    pub fn reset_all(&mut self) {
        for index in 0..THROTTLING_QUOTA_NUM {
            let (tx_max, rx_max) = THROTTLING_QUOTA_MAX[index];
            if self.quotas[index].0 < 0 {
                slog::warn!(
                    self.log,
                    "Tx quota is exceeded";
                    "msg_kind" => Self::index_to_str(index),
                    "quota" => tx_max,
                    "amount" => -self.quotas[index].0 + tx_max
                );
            }
            if self.quotas[index].1 < 0 {
                slog::warn!(
                    self.log,
                    "Rx quota is exceeded";
                    "msg_kind" => Self::index_to_str(index),
                    "quota" => rx_max,
                    "amount" => -self.quotas[index].1 + rx_max
                );
            }
            self.quotas[index].0 = tx_max;
            self.quotas[index].1 = rx_max;
        }
    }
}
