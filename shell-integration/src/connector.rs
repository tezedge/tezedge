// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Module contains strict/exact api between [shell] and  [outside thread/world, like rpc, ws...]
//! This covers one-direction "world -> shell" communication

use super::UnsupportedMessageError;
use crate::messages::*;

pub trait ShellConnector: InjectBlockConnector {
}

pub trait InjectBlockConnector {
    fn inject_block(
        &self,
        request: InjectBlock,
        result_callback: Option<InjectBlockOneshotResultCallback>,
    );
}

pub trait MempoolPrevalidatorCaller: Send {
    fn try_tell(&self, msg: MempoolRequestMessage) -> Result<(), UnsupportedMessageError>;
}

pub type ShellConnectorRef = Box<dyn ShellConnector + Sync + Send>;
