use serde::{Serialize, Deserialize};
use super::base_types::*;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct EndorsingRight {
    level: u64,
    //delegate: Option<UniString>,
    delegate: String,
    slots: Vec<u32>,
    estimated_time: String
}

pub type EndorsingRights = Vec<EndorsingRight>;