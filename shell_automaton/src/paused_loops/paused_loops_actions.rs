use serde::{Deserialize, Serialize};

use super::PausedLoop;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PausedLoopsAddAction {
    pub data: PausedLoop,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PausedLoopsResumeAllAction {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PausedLoopsResumeNextInitAction {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PausedLoopsResumeNextSuccessAction {}
