use crypto::hash::BlockHash;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;

const DB_PATH: &str = "context_stats.db";

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockStats {
    actions_count: usize,
    checkout_context_time: f64,
    commit_context_time: f64,
    operations_context: Vec<ActionStats>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct ActionData {
    root: String,
    mean_time: f64,
    max_time: f64,
    total_time: f64,
    actions_count: usize,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct ActionStats {
    data: ActionData,
    mem: f64,
    find: f64,
    find_tree: f64,
    add: f64,
    add_tree: f64,
    list: f64,
    fold: f64,
    remove: f64,
}

pub(crate) fn make_block_stats(block_hash: BlockHash) -> Result<BlockStats, failure::Error> {
    let sql = Connection::open(DB_PATH)?;
    make_block_stats_impl(sql, block_hash)
}

fn make_block_stats_impl(
    sql: Connection,
    block_hash: BlockHash,
) -> Result<BlockStats, failure::Error> {
    let block_hash = hash_to_string(block_hash.as_ref());

    let (block_id, actions_count, checkout_time, commit_time) = sql.query_row(
        "SELECT id, actions_count, checkout_time_tezedge, commit_time_tezedge FROM blocks WHERE hash = ?1",
        [block_hash],
        |row| {
            let block_id: usize = row.get(0)?;
            let actions_count: usize = row.get(1)?;
            let checkout_time: f64 = row.get(2)?;
            let commit_time: f64 = row.get(3)?;

            Ok((block_id, actions_count, checkout_time, commit_time))
        }
    )?;

    let mut stmt = sql.prepare(
        "
        SELECT
          key_root,
          name,
          avg(tezedge_time),
          total(tezedge_time),
          count(tezedge_time)
        FROM
          actions
        WHERE
          block_id = ?
        GROUP BY
          key_root,
          name
        ",
    )?;

    let mut rows = stmt.query([block_id])?;

    let mut map: HashMap<String, ActionStats> = HashMap::default();

    while let Some(row) = rows.next()? {
        let root: String = row.get(0)?;
        let action_name: String = row.get(1)?;
        let _mean: f64 = row.get(2)?;
        let total: f64 = row.get(3)?;
        let count: usize = row.get(4)?;

        let entry = map.entry(root.clone()).or_insert_with(|| ActionStats {
            data: ActionData {
                root,
                mean_time: 0.0,
                max_time: 0.0,
                total_time: 0.0,
                actions_count: 0,
            },
            mem: 0.0,
            find: 0.0,
            find_tree: 0.0,
            add: 0.0,
            add_tree: 0.0,
            list: 0.0,
            fold: 0.0,
            remove: 0.0,
        });

        match action_name.as_str() {
            "mem" => entry.mem = total,
            "find" => entry.find = total,
            "find_tree" => entry.find_tree = total,
            "add" => entry.add = total,
            "add_tree" => entry.add_tree = total,
            "list" => entry.list = total,
            "fold" => entry.fold = total,
            "remove" => entry.remove = total,
            _ => {}
        }

        entry.data.actions_count += count;
        entry.data.total_time += total;
    }

    for op in map.values_mut() {
        let mean = op.data.total_time / op.data.actions_count as f64;
        op.data.mean_time = mean.max(0.0);
    }

    Ok(BlockStats {
        actions_count,
        checkout_context_time: checkout_time,
        commit_context_time: commit_time,
        operations_context: map.into_iter().map(|(_, v)| v).collect(),
    })
}

pub fn hash_to_string(hash: &[u8]) -> String {
    const HEXCHARS: &[u8] = b"0123456789abcdef";

    let mut s = String::with_capacity(62);
    for byte in hash {
        s.push(HEXCHARS[*byte as usize >> 4] as char);
        s.push(HEXCHARS[*byte as usize & 0xF] as char);
    }
    s
}
