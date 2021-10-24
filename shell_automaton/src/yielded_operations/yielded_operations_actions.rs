use serde::{Deserialize, Serialize};

use super::YieldedOperation;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct YieldedOperationsAddAction {
    pub operation: YieldedOperation,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct YieldedOperationsExecuteAllAction {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct YieldedOperationsExecuteNextInitAction {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct YieldedOperationsExecuteNextSuccessAction {}
