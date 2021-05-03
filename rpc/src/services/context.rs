use serde::Serialize;
use std::{collections::HashMap, convert::TryInto};

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContextStats {
    actions_count: usize,
    checkout_context: CheckoutCommit,
    commit_context: CheckoutCommit,
    operations_context: Vec<BlockDetails>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckoutCommit {
    max: f64,
    mean: f64,
    total_time: f64,
}

/// Total time for each action in the block
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockDetails {
    block_hash: String,
    mem: f64,
    find: f64,
    find_tree: f64,
    add: f64,
    add_tree: f64,
    equal: f64,
    hash: f64,
    kind: f64,
    empty: f64,
    is_empty: f64,
    list: f64,
    fold: f64,
    remove: f64,
    commit: f64,
    checkout: f64,
    max: f64,
    mean: f64,
    total: f64,
}

pub(crate) fn make_context_stats() -> Result<ContextStats, failure::Error> {
    let sql = sqlite::open("context_timing.sql")?;

    let blocks = read_blocks(&sql);
    let blocks = get_block_details(&sql, blocks)?;

    let mut stats = get_context_stats(&sql)?.unwrap_or_default();

    stats.operations_context = blocks;

    println!("GLOBAL HERE = {:#?}", stats);

    Ok(stats)
}

fn get_context_stats(sql: &sqlite::Connection) -> Result<Option<ContextStats>, failure::Error> {
    let mut cursor = sql
        .prepare(
            "SELECT
           actions_count,
           tezedge_checkouts_max,
           tezedge_checkouts_mean,
           tezedge_checkouts_total,
           tezedge_commits_max,
           tezedge_commits_mean,
           tezedge_commits_total
         FROM
           global_stats",
        )
        .unwrap()
        .into_cursor();

    let row = match cursor.next()? {
        Some(row) => row,
        None => return Ok(None),
    };

    let stats = ContextStats {
        actions_count: row[0].as_integer().unwrap().try_into().unwrap(),
        checkout_context: CheckoutCommit {
            max: row[1].as_float().unwrap(),
            mean: row[2].as_float().unwrap(),
            total_time: row[3].as_float().unwrap(),
        },
        commit_context: CheckoutCommit {
            max: row[4].as_float().unwrap(),
            mean: row[5].as_float().unwrap(),
            total_time: row[6].as_float().unwrap(),
        },
        operations_context: Vec::new(),
    };

    return Ok(Some(stats));
}

fn get_block_details(
    sql: &sqlite::Connection,
    mut blocks_map: HashMap<String, BlockDetails>,
) -> Result<Vec<BlockDetails>, failure::Error> {
    let mut cursor = sql
        .prepare(
            "SELECT
            block_details.action_name,
            block_details.tezedge_time,
            blocks.hash AS block_hash
         FROM
            block_details
         JOIN blocks
         WHERE
            block_id = blocks.id",
        )?
        .into_cursor();

    while let Some(row) = cursor.next()? {
        let block_hash = match row[2].as_string() {
            Some(hash) => hash.to_string(),
            None => continue,
        };

        let entry = match blocks_map.get_mut(&block_hash) {
            Some(entry) => entry,
            _ => continue,
        };

        let action_name = row[0].as_string().unwrap();
        let value = row[1].as_float().unwrap_or(0.0);

        match action_name {
            "mem" => entry.mem = value,
            "find" => entry.find = value,
            "find_tree" => entry.find_tree = value,
            "add" => entry.add = value,
            "add_tree" => entry.add_tree = value,
            "equal" => entry.equal = value,
            "hash" => entry.hash = value,
            "kind" => entry.kind = value,
            "empty" => entry.empty = value,
            "is_empty" => entry.is_empty = value,
            "list" => entry.list = value,
            "fold" => entry.fold = value,
            "remove" => entry.remove = value,
            "commit" => entry.commit = value,
            "checkout" => entry.checkout = value,
            _ => {}
        }
    }

    Ok(blocks_map
        .into_iter()
        .map(|(_hash, details)| details)
        .collect())
}

fn read_blocks(sql: &sqlite::Connection) -> HashMap<String, BlockDetails> {
    let mut blocks_map: HashMap<String, BlockDetails> = HashMap::new();

    let mut cursor = sql
        .prepare(
            "SELECT
            hash,
            tezedge_time_max,
            tezedge_time_mean,
            tezedge_time_total
         FROM
            blocks;",
        )
        .unwrap()
        .into_cursor();

    while let Some(row) = cursor.next().unwrap() {
        let block_hash = row[0].as_string().unwrap();

        let mut details = BlockDetails::default();
        details.block_hash = block_hash.to_string();
        details.max = row[1].as_float().unwrap_or(0.0);
        details.mean = row[2].as_float().unwrap_or(0.0);
        details.total = row[3].as_float().unwrap_or(0.0);

        blocks_map.insert(block_hash.to_string(), details);
    }

    blocks_map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_db() {
        make_context_stats().unwrap();
    }
}
