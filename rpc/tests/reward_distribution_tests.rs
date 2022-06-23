// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Integration test that compares the node's reward calculations to the one of an indexer (currently only tzkt as tzstats seems to have a bug in it's rewards' response)
//!
//! Note: You require to run a synched TezEdge node on ithacanet
//!
//! usage:
//!
//! ```
//!     FROM_CYCLE=100 TO_CYCLE=153 NODE_RPC_CONTEXT_ROOT_1=http://116.202.128.230:18732 cargo test --release reward_distribution_tests -- --ignored --nocapture test_rewards_distribution
//! ```

use std::collections::BTreeMap;
use std::env;
use std::str::FromStr;
use std::time::Instant;

use anyhow::format_err;
use hyper::body::Buf;
use hyper::Client;
use hyper_tls::HttpsConnector;
use num::{BigInt, BigRational};
use serde::{de::DeserializeOwned, Deserialize};
use url::Url;

fn client() -> Client<hyper::client::HttpConnector, hyper::Body> {
    Client::new()
}

fn node_rpc_context_root_1() -> String {
    let node_url = env::var("NODE_RPC_CONTEXT_ROOT_1")
        .expect("env variable 'NODE_RPC_CONTEXT_ROOT_1' should be set");
    Url::parse(&node_url).expect("invalid url").to_string()
}

fn from_cycle() -> i32 {
    env::var("FROM_CYCLE")
        .unwrap_or_else(|_| panic!("FROM_CYCLE env variable is missing"))
        .parse()
        .unwrap_or_else(|_| panic!("FROM_CYCLE env variable can not be parsed as a number"))
}

fn to_cycle() -> i32 {
    env::var("TO_CYCLE")
        .unwrap_or_else(|_| panic!("TO_CYCLE env variable is missing"))
        .parse()
        .unwrap_or_else(|_| panic!("TO_CYCLE env variable can not be parsed as a number"))
}

#[ignore]
#[tokio::test]
async fn test_rewards_distribution() {
    let cycles = from_cycle()..=to_cycle();

    let tezedge_url = node_rpc_context_root_1();

    // Tzkt
    for cycle in cycles {
        let tezedge_delegate_rewards: TezedgeDelegateRewardsResponse =
            call_tezedge_rpc(&format!("dev/rewards/cycle/{}", cycle), &tezedge_url)
                .await
                .unwrap();

        for delegate in tezedge_delegate_rewards {
            println!("Comparing cycle {cycle} delegate {}", delegate.address);
            let tezedge_response: TezedgeDelegateRewardDistribution = call_tezedge_rpc(
                &format!("dev/rewards/cycle/{}/{}", cycle, delegate.address),
                &tezedge_url,
            )
            .await
            .unwrap();
            let tzkt_response = get_tzkt_rewards_split(cycle, &delegate.address)
                .await
                .unwrap();

            compare_rewards(tzkt_response, tezedge_response.delegator_rewards);
        }

        println!();
    }

    // Note: Enable once tzstats bug is resolved
    // Tzstats
    // for cycle in cycles {
    //     if tzstats_exclude.contains(&cycle) {
    //         println!("Excluding {cycle}");
    //     } else {
    //         let tezedge_reward = get_tezedge_rewards(cycle).await.unwrap();
    //         let delegates: Vec<String> = tezedge_reward.iter().map(|v| v.address.clone()).collect();

    //         for (idx, delegate) in delegates.iter().enumerate() {

    //             // if exclude_delegates.contains(&delegate.as_str()) {
    //             //     println!("Excluding delegate {}", delegate);
    //             //     continue;
    //             // }

    //             // Tzstats limits requests
    //             if idx % 10 == 0 {
    //                 println!("Waiting for tzstats...");
    //                 tokio::time::sleep(Duration::from_secs(1)).await;
    //             }

    //             println!("Comparing cycle {cycle} delegate {delegate}");
    //             let tzstats_response = get_tzstats_rewards(cycle, delegate).await.unwrap();
    //             let tezedge_delegate_split = tezedge_reward.get(delegate).unwrap();

    //             compare_rewards(tzstats_response, &tezedge_delegate_split.delegator_rewards);
    //         }
    //     }

    //     println!();
    // }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TzktRewardSplitResponse {
    staking_balance: i64,
    block_rewards: i64,
    block_fees: i64,
    endorsement_rewards: i64,
    double_baking_rewards: i64,
    double_endorsing_rewards: i64,
    double_preendorsing_rewards: i64,
    revelation_rewards: i64,
    delegators: Vec<IndexerDelegatorInfo>,
}

#[derive(Clone, Debug, Deserialize)]
struct IndexerDelegatorInfo {
    address: String,
    balance: i64,
}

#[derive(Clone, Debug, Deserialize)]
struct TzstatsRewardsResponse {
    total_income: i64,
    staking_balance: i64,
    delegators: Vec<IndexerDelegatorInfo>,
}

// type TezedgeDelegatorRewardsResponse = Vec<TezedgeDelegateRewardDistribution>;
type TezedgeDelegateRewardsResponse = Vec<TezedgeDelegateRewards>;

#[derive(Clone, Debug, Deserialize)]
struct TezedgeDelegateRewards {
    address: String,
    // reward: String,
}

#[derive(Clone, Debug, Deserialize)]
struct TezedgeDelegatorRewards {
    address: String,
    reward: String,
}

#[derive(Clone, Debug, Deserialize)]
struct TezedgeDelegateRewardDistribution {
    delegator_rewards: Vec<TezedgeDelegatorRewards>,
}

trait IndexerReward {
    fn total_rewards(&self) -> i64;
    fn staking_balance(&self) -> i64;
    fn delegators(&self) -> Vec<IndexerDelegatorInfo>;
}

impl IndexerReward for TzktRewardSplitResponse {
    fn total_rewards(&self) -> i64 {
        self.block_rewards
            + self.block_fees
            + self.endorsement_rewards
            + self.double_baking_rewards
            + self.double_endorsing_rewards
            + self.double_preendorsing_rewards
            + self.revelation_rewards
    }

    fn staking_balance(&self) -> i64 {
        self.staking_balance
    }

    fn delegators(&self) -> Vec<IndexerDelegatorInfo> {
        self.delegators.clone()
    }
}

impl IndexerReward for TzstatsRewardsResponse {
    fn total_rewards(&self) -> i64 {
        self.total_income
        // self.total_income - self.total_loss
    }

    fn staking_balance(&self) -> i64 {
        self.staking_balance
    }

    fn delegators(&self) -> Vec<IndexerDelegatorInfo> {
        self.delegators.clone()
    }
}

fn compare_rewards<T: IndexerReward>(
    indexer_rewards: T,
    tezedge_delegator_rewards: Vec<TezedgeDelegatorRewards>,
) {
    let indexer_delegate_reward = indexer_rewards.total_rewards();
    let indexer_staking_balance = indexer_rewards.staking_balance();

    let tezedge_delegator_rewards_int: BTreeMap<String, BigInt> = tezedge_delegator_rewards
        .iter()
        .map(|reward| {
            (
                reward.address.clone(),
                BigInt::from_str(&reward.reward).unwrap(),
            )
        })
        .collect();

    let indexer_delegator_rewards: BTreeMap<String, BigInt> = indexer_rewards
        .delegators()
        .iter()
        .map(|delegator| {
            (
                delegator.address.clone(),
                calculate_reward_share(
                    indexer_staking_balance,
                    indexer_delegate_reward,
                    delegator.balance,
                ),
            )
        })
        .collect();

    // if indexer_delegator_rewards.is_empty() && !tezedge_delegator_rewards_int.is_empty() {
    //     println!("Indexer returned no delegators, skipping...")
    // } else {
    //     assert_eq!(tezedge_delegator_rewards_int, indexer_delegator_rewards);
    // }

    if indexer_delegator_rewards.len() != tezedge_delegator_rewards_int.len() {
        println!(
            "Delegator count difference - tezedge({}) indexer({})",
            tezedge_delegator_rewards_int.len(),
            indexer_delegator_rewards.len()
        )
    } else {
        assert_eq!(tezedge_delegator_rewards_int, indexer_delegator_rewards);
    }

    // println!("Comparing cycle {} delegate {}")
    // let tzkt_delegate_reward = tzkt_rewards.delegators.iter().map(|delegate|)
}

fn calculate_reward_share(
    staking_balance: i64,
    total_rewards: i64,
    delegator_balance: i64,
) -> BigInt {
    let staking_balance = BigInt::from(staking_balance);
    let total_rewards = BigInt::from(total_rewards);
    let delegator_balance = BigInt::from(delegator_balance);

    let share = BigRational::new(delegator_balance, staking_balance);

    let reward = BigRational::from(total_rewards) * share;

    reward.round().to_integer()
}

async fn get_tzkt_rewards_split(
    cycle: i32,
    delegate: &str,
) -> Result<TzktRewardSplitResponse, anyhow::Error> {
    // https://api.ithacanet.tzkt.io/v1/rewards/split/tz1RuHDSj9P7mNNhfKxsyLGRDahTX5QD1DdP/124
    // const TZKT_API_SPLIT_ENDPOINT: &str = "https://api.tzkt.io/v1/rewards/split";
    const TZKT_API_SPLIT_ENDPOINT: &str = "https://api.ithacanet.tzkt.io/v1/rewards/split";
    // const TZKT_API_SPLIT_ENDPOINT: &str = "https://api.jakartanet.tzkt.io/v1/rewards/split";

    let url_as_string = format!(
        "{}/{}/{}?limit=10000",
        TZKT_API_SPLIT_ENDPOINT, delegate, cycle
    );
    let url = url_as_string
        .parse()
        .unwrap_or_else(|_| panic!("Invalid URL: {}", &url_as_string));

    let https = HttpsConnector::new();
    let client = hyper::Client::builder().build::<_, hyper::Body>(https);
    let start = Instant::now();
    let (status_code, body, _) = match client.get(url).await {
        Ok(res) => {
            let finished = start.elapsed();
            (
                res.status(),
                hyper::body::aggregate(res.into_body()).await.expect("Failed to read response body"),
                finished,
            )
        },
        Err(e) => return Err(format_err!("Request url: {:?} for getting data failed: {} - please, check node's log, in the case of network or connection error, please, check rpc/README.md for CONTEXT_ROOT configurations", url_as_string, e)),
    };

    // process response body
    let mut buf = body.reader();
    let mut dst = vec![];
    std::io::copy(&mut buf, &mut dst).unwrap();

    // process status code
    if status_code.is_success() {
        let response_value: TzktRewardSplitResponse = match serde_json::from_slice(&dst) {
            Ok(result) => result,
            Err(err) => {
                return Err(format_err!(
                    "Error {:?} when parsing value as JSON: {:?}",
                    err,
                    String::from_utf8_lossy(&dst)
                ))
            }
        };
        Ok(response_value)
        // Ok((status_code, response_value, response_time))
    } else {
        panic!("Request failed")
    }
}

// TODO: duplicate body...
async fn call_tezedge_rpc<T: DeserializeOwned>(
    endpoint: &str,
    node_url: &str,
) -> Result<T, anyhow::Error> {
    // const TEZEDGE_REWARDS_RPC_ENDOPOINT: &str = "dev/rewards/cycle";

    let url_as_string = format!("{}{}", node_url, endpoint);
    let url = url_as_string
        .parse()
        .unwrap_or_else(|_| panic!("Invalid URL: {}", &url_as_string));

    let client = client();
    let start = Instant::now();
    let (status_code, body, _) = match client.get(url).await {
        Ok(res) => {
            let finished = start.elapsed();
            (
                res.status(),
                hyper::body::aggregate(res.into_body()).await.expect("Failed to read response body"),
                finished,
            )
        },
        Err(e) => return Err(format_err!("Request url: {:?} for getting data failed: {} - please, check node's log, in the case of network or connection error, please, check rpc/README.md for CONTEXT_ROOT configurations", url_as_string, e)),
    };

    // process response body
    let mut buf = body.reader();
    let mut dst = vec![];
    std::io::copy(&mut buf, &mut dst).unwrap();

    // process status code
    if status_code.is_success() {
        let response_value: T = match serde_json::from_slice(&dst) {
            Ok(result) => result,
            Err(err) => {
                return Err(format_err!(
                    "Error {:?} when parsing value as JSON: {:?}",
                    err,
                    String::from_utf8_lossy(&dst)
                ))
            }
        };
        Ok(response_value)
        // Ok((status_code, response_value, response_time))
    } else {
        panic!("Request failed")
    }
}

// TODO: duplicate body...
#[allow(dead_code)]
async fn get_tzstats_rewards(
    cycle: i32,
    delegate: &str,
) -> Result<TzstatsRewardsResponse, anyhow::Error> {
    // https://api.ithacanet.tzkt.io/v1/rewards/split/tz1RuHDSj9P7mNNhfKxsyLGRDahTX5QD1DdP/124
    const TZSTATS_REWARDS_RPC_ENDOPOINT: &str = "https://api.ithaca.tzstats.com/explorer/bakers";

    let url_as_string = format!(
        "{}/{}/snapshot/{}",
        TZSTATS_REWARDS_RPC_ENDOPOINT, delegate, cycle
    );
    let url = url_as_string
        .parse()
        .unwrap_or_else(|_| panic!("Invalid URL: {}", &url_as_string));

    let https = HttpsConnector::new();
    let client = hyper::Client::builder().build::<_, hyper::Body>(https);
    let start = Instant::now();
    let (status_code, body, _) = match client.get(url).await {
        Ok(res) => {
            let finished = start.elapsed();
            (
                res.status(),
                hyper::body::aggregate(res.into_body()).await.expect("Failed to read response body"),
                finished,
            )
        },
        Err(e) => return Err(format_err!("Request url: {:?} for getting data failed: {} - please, check node's log, in the case of network or connection error, please, check rpc/README.md for CONTEXT_ROOT configurations", url_as_string, e)),
    };

    // process response body
    let mut buf = body.reader();
    let mut dst = vec![];
    std::io::copy(&mut buf, &mut dst).unwrap();

    // process status code
    if status_code.is_success() {
        let response_value: TzstatsRewardsResponse = match serde_json::from_slice(&dst) {
            Ok(result) => result,
            Err(err) => {
                return Err(format_err!(
                    "Error {:?} when parsing value as JSON: {:?}",
                    err,
                    String::from_utf8_lossy(&dst)
                ))
            }
        };
        Ok(response_value)
        // Ok((status_code, response_value, response_time))
    } else {
        panic!(
            "Request failed with status: {}\nBody: {:#?}",
            status_code,
            String::from_utf8_lossy(&dst)
        )
    }
}
