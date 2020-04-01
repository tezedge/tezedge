// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use serde::Serialize;
use getset::Getters;

use crate::protocol::{ToRpcJsonMap, UniversalValue, ToRpcJsonList};
use tezos_encoding::types::BigInt;

#[derive(Serialize, Getters, Debug, Clone)]
pub struct BalanceByCycle {
    cycle: i32,
    #[get = "pub"]
    deposit: BigInt,
    #[get = "pub"]
    fees: BigInt,
    #[get = "pub"]
    rewards: BigInt,
}

impl BalanceByCycle {
    pub fn new(
        cycle: i32,
        deposit: BigInt,
        fees: BigInt,
        rewards: BigInt,
    ) -> Self {
        Self {
            cycle,
            deposit,
            fees,
            rewards,
        }
    }
}

impl ToRpcJsonMap for BalanceByCycle {
    fn as_map(&self) -> HashMap<&'static str, UniversalValue> {
        let mut ret: HashMap<&'static str, UniversalValue> = Default::default();
        ret.insert("cycle", UniversalValue::num(self.cycle));
        ret.insert("deposit", UniversalValue::big_num(self.deposit.clone()));
        ret.insert("fees", UniversalValue::big_num(self.fees.clone()));
        ret.insert("rewards", UniversalValue::big_num(self.rewards.clone()));
        ret
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct Delegate {
    balance: BigInt,
    frozen_balance: BigInt,
    frozen_balance_by_cycle: Vec<BalanceByCycle>,
    staking_balance: BigInt,
    delegated_contracts: Vec<String>,
    delegated_balance: BigInt,
    deactivated: bool,
    grace_period: i32,
}

impl Delegate {
    /// Simple constructor to construct VoteListings
    pub fn new(
        balance: BigInt,
        frozen_balance: BigInt,
        frozen_balance_by_cycle: Vec<BalanceByCycle>,
        staking_balance: BigInt,
        delegated_contracts: Vec<String>,
        delegated_balance: BigInt,
        deactivated: bool,
        grace_period: i32,
    ) -> Self {
        Self {
            balance,
            frozen_balance,
            frozen_balance_by_cycle,
            staking_balance,
            delegated_contracts,
            delegated_balance,
            deactivated,
            grace_period
        }
    }
}

impl ToRpcJsonMap for Delegate {
    fn as_map(&self) -> HashMap<&'static str, UniversalValue> {
        let mut ret: HashMap<&'static str, UniversalValue> = Default::default();
        ret.insert("balance", UniversalValue::big_num(self.balance.clone()));
        ret.insert("frozen_balance", UniversalValue::big_num(self.frozen_balance.clone()));
        let frozen_balance_by_cycle_map: Vec<HashMap<&'static str, UniversalValue>> = self.frozen_balance_by_cycle.clone()
            .into_iter()
            .map(|val| val.as_map())
            .collect();

        ret.insert("frozen_balance_by_cycle", UniversalValue::map_list(frozen_balance_by_cycle_map));
        ret.insert("staking_balance", UniversalValue::big_num(self.staking_balance.clone()));
        ret.insert("delegated_contracts", UniversalValue::string_list(self.delegated_contracts.clone()));
        ret.insert("delegated_balance", UniversalValue::big_num(self.delegated_balance.clone()));
        ret.insert("deactivated", UniversalValue::bool(self.deactivated));
        ret.insert("grace_period", UniversalValue::num(self.grace_period));

        ret
    }
}

pub type DelegateList = Vec<String>;

impl ToRpcJsonList for DelegateList {
    fn as_list(&self) -> UniversalValue {
        UniversalValue::string_list(self.clone())
    }
}