// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Module contains strict/exact api between [shell] and  [outside thread/world, like rpc, ws...]
//! This covers one-direction "world -> shell" communication

use crypto::hash::ChainId;

use super::{UnexpectedError, UnsupportedMessageError};
use crate::messages::*;

pub trait ShellConnector: InjectBlockConnector {
    fn request_current_head_from_connected_peers(&self);

    fn find_mempool_prevalidators(&self) -> Result<Vec<Prevalidator>, UnexpectedError>;

    fn find_mempool_prevalidator_caller(
        &self,
        chain_id: &ChainId,
    ) -> Option<Box<dyn MempoolPrevalidatorCaller>>;
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
