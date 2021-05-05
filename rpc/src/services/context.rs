use serde::Serialize;
use std::{collections::HashMap, convert::TryInto};
use rusqlite::Connection;

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContextStats {
    actions_count: usize,
    checkout_context: TimeStats,
    commit_context: TimeStats,
    operations_context: Vec<BlockStats>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TimeStats {
    max: f64,
    mean: f64,
    total_time: f64,
}

/// Total time for each action in the block
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockStats {
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
    let sql = Connection::open("context_stats.db")?;

    let blocks = get_blocks_stats(&sql)?;
    let blocks = get_actions_stats(&sql, blocks)?;

    let stats = get_context_stats(&sql, blocks)?.unwrap_or_default();

    Ok(stats)
}

fn get_context_stats(
    sql: &Connection,
    blocks: Vec<BlockStats>,
) -> Result<Option<ContextStats>, failure::Error> {
    let mut stmt = sql
        .prepare(
            "
         SELECT
           actions_count,
           tezedge_checkouts_max,
           tezedge_checkouts_mean,
           tezedge_checkouts_total,
           tezedge_commits_max,
           tezedge_commits_mean,
           tezedge_commits_total
         FROM
           global_stats
         WHERE
           id = 0;
            ",
        )?;

    let mut rows = stmt.query([])?;

    let row = match rows.next()? {
        Some(row) => row,
        None => return Ok(None),
    };

    let stats = ContextStats {
        actions_count: row.get(0).unwrap_or(0).try_into()?,
        checkout_context: TimeStats {
            max: row.get(1).unwrap_or(0.0),
            mean: row.get(2).unwrap_or(0.0),
            total_time: row.get(3).unwrap_or(0.0),
        },
        commit_context: TimeStats {
            max: row.get(4).unwrap_or(0.0),
            mean: row.get(5).unwrap_or(0.0),
            total_time: row.get(6).unwrap_or(0.0),
        },
        operations_context: blocks,
    };

    return Ok(Some(stats));
}

fn get_actions_stats(
    sql: &Connection,
    mut blocks_map: HashMap<String, BlockStats>,
) -> Result<Vec<BlockStats>, failure::Error> {
    let mut stmt = sql
        .prepare(
            "
         SELECT
            block_details.action_name,
            block_details.tezedge_time,
            blocks.hash AS block_hash
         FROM
            block_details
         JOIN blocks ON block_details.block_id = blocks.id
            ",
        )?;

    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let block_hash: String = match row.get(2).unwrap_or(None) {
            Some(hash) => hash,
            None => continue,
        };

        let block = match blocks_map.get_mut(&block_hash) {
            Some(block) => block,
            _ => continue,
        };

        let action_name: String = match row.get(0).unwrap_or(None) {
            Some(name) => name,
            None => continue
        };

        let value = row.get(1).unwrap_or(0.0);

        match action_name.as_str() {
            "mem" => block.mem = value,
            "find" => block.find = value,
            "find_tree" => block.find_tree = value,
            "add" => block.add = value,
            "add_tree" => block.add_tree = value,
            "equal" => block.equal = value,
            "hash" => block.hash = value,
            "kind" => block.kind = value,
            "empty" => block.empty = value,
            "is_empty" => block.is_empty = value,
            "list" => block.list = value,
            "fold" => block.fold = value,
            "remove" => block.remove = value,
            "commit" => block.commit = value,
            "checkout" => block.checkout = value,
            _ => {}
        }
    }

    Ok(blocks_map
        .into_iter()
        .map(|(_hash, details)| details)
        .collect())
}

fn get_blocks_stats(
    sql: &Connection,
) -> Result<HashMap<String, BlockStats>, failure::Error> {
    let mut blocks_map: HashMap<String, BlockStats> = HashMap::new();

    let mut stmt = sql
        .prepare(
            "SELECT
            hash,
            tezedge_time_max,
            tezedge_time_mean,
            tezedge_time_total
         FROM
            blocks;",
        )?;

    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let block_hash: String = row.get(0).unwrap();

        let mut block = BlockStats::default();
        block.block_hash = block_hash.clone();
        block.max = row.get(1).unwrap_or(0.0);
        block.mean = row.get(2).unwrap_or(0.0);
        block.total = row.get(3).unwrap_or(0.0);

        blocks_map.insert(block_hash, block);
    }

    Ok(blocks_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_db() {
        make_context_stats().unwrap();
    }
}
