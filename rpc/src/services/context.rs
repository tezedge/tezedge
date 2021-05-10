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
    remove: f64,
}

impl ActionStats {
    fn compute_mean(&mut self) {
        let mean = self.data.total_time / self.data.actions_count as f64;
        self.data.mean_time = mean.max(0.0);
    }
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContextStats {
    operations_context: Vec<ActionStatsWithRange>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct DetailedTime {
    count: usize,
    mean_time: f64,
    max_time: f64,
    total_time: f64,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct RangeStats {
    one_to_ten_us: DetailedTime,
    ten_to_one_hundred_us: DetailedTime,
    one_hundred_us_to_one_ms: DetailedTime,
    one_to_ten_ms: DetailedTime,
    ten_to_one_hundred_ms: DetailedTime,
    one_hundred_ms_to_one_s: DetailedTime,
    one_to_ten_s: DetailedTime,
    ten_to_one_hundred_s: DetailedTime,
    one_hundred_s: DetailedTime,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct ActionStatsWithRange {
    root: String,
    mem: RangeStats,
    find: RangeStats,
    find_tree: RangeStats,
    add: RangeStats,
    add_tree: RangeStats,
    remove: RangeStats,
}

pub(crate) fn make_block_stats(block_hash: BlockHash) -> Result<BlockStats, failure::Error> {
    let sql = Connection::open(DB_PATH)?;
    make_block_stats_impl(&sql, block_hash)
}

pub(crate) fn make_context_stats() -> Result<ContextStats, failure::Error> {
    let sql = Connection::open(DB_PATH)?;
    make_context_stats_impl(&sql)
}

fn make_context_stats_impl(sql: &Connection) -> Result<ContextStats, failure::Error> {
    let mut stmt = sql.prepare(
        "
    SELECT
      action_name,
      root,
      one_to_ten_us_count,
      one_to_ten_us_mean_time,
      one_to_ten_us_max_time,
      one_to_ten_us_total_time,
      ten_to_one_hundred_us_count,
      ten_to_one_hundred_us_mean_time,
      ten_to_one_hundred_us_max_time,
      ten_to_one_hundred_us_total_time,
      one_hundred_us_to_one_ms_count,
      one_hundred_us_to_one_ms_mean_time,
      one_hundred_us_to_one_ms_max_time,
      one_hundred_us_to_one_ms_total_time,
      one_to_ten_ms_count,
      one_to_ten_ms_mean_time,
      one_to_ten_ms_max_time,
      one_to_ten_ms_total_time,
      ten_to_one_hundred_ms_count,
      ten_to_one_hundred_ms_mean_time,
      ten_to_one_hundred_ms_max_time,
      ten_to_one_hundred_ms_total_time,
      one_hundred_ms_to_one_s_count,
      one_hundred_ms_to_one_s_mean_time,
      one_hundred_ms_to_one_s_max_time,
      one_hundred_ms_to_one_s_total_time,
      one_to_ten_s_count,
      one_to_ten_s_mean_time,
      one_to_ten_s_max_time,
      one_to_ten_s_total_time,
      ten_to_one_hundred_s_count,
      ten_to_one_hundred_s_mean_time,
      ten_to_one_hundred_s_max_time,
      ten_to_one_hundred_s_total_time,
      one_hundred_s_count,
      one_hundred_s_mean_time,
      one_hundred_s_max_time,
      one_hundred_s_total_time
    FROM global_range_stats;
       ",
    )?;

    let mut rows = stmt.query([])?;

    let mut map: HashMap<String, ActionStatsWithRange> = HashMap::default();

    while let Some(row) = rows.next()? {
        let action_name = match row.get_ref(0)?.as_str() {
            Ok(name) if !name.is_empty() => name,
            _ => continue,
        };

        let root = match row.get_ref(1)?.as_str() {
            Ok(root) if !root.is_empty() => root,
            _ => continue,
        };

        let entry = match map.get_mut(root) {
            Some(entry) => entry,
            None => {
                let mut stats = ActionStatsWithRange::default();
                stats.root = root.to_string();
                map.insert(root.to_string(), stats);
                map.get_mut(root).unwrap()
            }
        };

        let range_stats = match action_name {
            "mem" => &mut entry.mem,
            "find" => &mut entry.find,
            "find_tree" => &mut entry.find_tree,
            "add" => &mut entry.add,
            "add_tree" => &mut entry.add_tree,
            "remove" => &mut entry.remove,
            _ => continue,
        };

        range_stats.one_to_ten_us.count = row.get(2)?;
        range_stats.one_to_ten_us.mean_time = row.get(3)?;
        range_stats.one_to_ten_us.max_time = row.get(4)?;
        range_stats.one_to_ten_us.total_time = row.get(5)?;
        range_stats.ten_to_one_hundred_us.count = row.get(6)?;
        range_stats.ten_to_one_hundred_us.mean_time = row.get(7)?;
        range_stats.ten_to_one_hundred_us.max_time = row.get(8)?;
        range_stats.ten_to_one_hundred_us.total_time = row.get(9)?;
        range_stats.one_hundred_us_to_one_ms.count = row.get(10)?;
        range_stats.one_hundred_us_to_one_ms.mean_time = row.get(11)?;
        range_stats.one_hundred_us_to_one_ms.max_time = row.get(12)?;
        range_stats.one_hundred_us_to_one_ms.total_time = row.get(13)?;
        range_stats.one_to_ten_ms.count = row.get(14)?;
        range_stats.one_to_ten_ms.mean_time = row.get(15)?;
        range_stats.one_to_ten_ms.max_time = row.get(16)?;
        range_stats.one_to_ten_ms.total_time = row.get(17)?;
        range_stats.ten_to_one_hundred_ms.count = row.get(18)?;
        range_stats.ten_to_one_hundred_ms.mean_time = row.get(19)?;
        range_stats.ten_to_one_hundred_ms.max_time = row.get(20)?;
        range_stats.ten_to_one_hundred_ms.total_time = row.get(21)?;
        range_stats.one_hundred_ms_to_one_s.count = row.get(22)?;
        range_stats.one_hundred_ms_to_one_s.mean_time = row.get(23)?;
        range_stats.one_hundred_ms_to_one_s.max_time = row.get(24)?;
        range_stats.one_hundred_ms_to_one_s.total_time = row.get(25)?;
        range_stats.one_to_ten_s.count = row.get(26)?;
        range_stats.one_to_ten_s.mean_time = row.get(27)?;
        range_stats.one_to_ten_s.max_time = row.get(28)?;
        range_stats.one_to_ten_s.total_time = row.get(29)?;
        range_stats.ten_to_one_hundred_s.count = row.get(30)?;
        range_stats.ten_to_one_hundred_s.mean_time = row.get(31)?;
        range_stats.ten_to_one_hundred_s.max_time = row.get(32)?;
        range_stats.ten_to_one_hundred_s.total_time = row.get(33)?;
        range_stats.one_hundred_s.count = row.get(34)?;
        range_stats.one_hundred_s.mean_time = row.get(35)?;
        range_stats.one_hundred_s.max_time = row.get(36)?;
        range_stats.one_hundred_s.total_time = row.get(37)?;
    }

    Ok(ContextStats {
        operations_context: map.into_iter().map(|(_, v)| v).collect(),
    })
}

fn make_block_stats_impl(
    sql: &Connection,
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
          total(tezedge_time),
          count(tezedge_time),
          max(tezedge_time)
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
        let root = match row.get_ref(0)?.as_str() {
            Ok(root) if !root.is_empty() => root,
            _ => continue,
        };

        let action_name = match row.get_ref(1)?.as_str() {
            Ok(name) if !name.is_empty() => name,
            _ => continue,
        };

        let total: f64 = row.get(2)?;
        let count: usize = row.get(3)?;
        let max_time: f64 = row.get(4)?;

        let entry = match map.get_mut(root) {
            Some(entry) => entry,
            None => {
                let mut stats = ActionStats::default();
                stats.data.root = root.to_string();
                map.insert(root.to_string(), stats);
                map.get_mut(root).unwrap()
            }
        };

        match action_name {
            "mem" => entry.mem = total,
            "find" => entry.find = total,
            "find_tree" => entry.find_tree = total,
            "add" => entry.add = total,
            "add_tree" => entry.add_tree = total,
            "remove" => entry.remove = total,
            _ => {}
        }

        entry.data.actions_count += count;
        entry.data.total_time += total;
        entry.data.max_time = entry.data.max_time.max(max_time);
    }

    for action in map.values_mut() {
        action.compute_mean();
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

#[cfg(test)]
mod tests {
    use crypto::hash::HashTrait;
    use rusqlite::Batch;

    use super::*;

    #[test]
    fn test_read_db() {
        let sql = Connection::open_in_memory().unwrap();

        let block_hash = BlockHash::try_from_bytes(&vec![1; 32]).unwrap();
        let block_hash_str = hash_to_string(block_hash.as_ref());

        let schema = include_str!("../../../tezos/new_context/src/schema_stats.sql");
        let mut batch = Batch::new(&sql, schema);
        while let Some(mut stmt) = batch.next().unwrap() {
            stmt.execute([]).unwrap();
        }

        sql.execute(
            "
            INSERT INTO blocks
               (id, hash, actions_count, checkout_time_tezedge, commit_time_tezedge)
            VALUES
               (1, ?1, 4, 10.0, 11.0);",
            [block_hash_str],
        )
        .unwrap();

        sql.execute(
            "
        INSERT INTO actions
          (name, key_root, key, irmin_time, tezedge_time, block_id, operation_id, context_id)
        VALUES
          ('mem', 'a' ,'a/b/c', 1.2, 1.3, 1, NULL, NULL),
          ('mem', 'a' ,'a/b/d', 5.2, 5.3, 1, NULL, NULL),
          ('add', 'a' ,'a/b/c/d', 1.5, 1.6, 1, NULL, NULL),
          ('add', 'm' ,'m/n/o', 1.5, 1.6, 1, NULL, NULL);
            ",
            [],
        )
        .unwrap();

        let block_stats = make_block_stats_impl(&sql, block_hash).unwrap();

        assert_eq!(block_stats.actions_count, 4);
        assert_eq!(block_stats.checkout_context_time, 10.0);
        assert_eq!(block_stats.commit_context_time, 11.0);
        assert_eq!(block_stats.operations_context.len(), 2);

        let action = block_stats
            .operations_context
            .iter()
            .find(|a| a.data.root == "a")
            .unwrap();
        assert_eq!(action.data.root, "a");
        assert_eq!(action.data.mean_time, 2.733333333333333);
        assert_eq!(action.add, 1.6);
        assert_eq!(action.find_tree, 0.0);

        sql.execute(
            "
        INSERT INTO global_range_stats
          (action_name, root, one_to_ten_us_count, one_to_ten_us_mean_time, one_to_ten_us_max_time, one_to_ten_us_total_time)
        VALUES
          ('mem', 'a', 2, 1.3, 1.4, 1.5),
          ('mem', 'b', 3, 10.3, 10.4, 10.5),
          ('add', 'b', 4, 20.3, 20.4, 20.5);
            ",
            [],
        )
        .unwrap();

        let context_stats = make_context_stats_impl(&sql).unwrap();

        assert_eq!(context_stats.operations_context.len(), 2);

        let action = context_stats
            .operations_context
            .iter()
            .find(|a| a.root == "a")
            .unwrap();
        assert_eq!(action.mem.one_to_ten_us.mean_time, 1.3);
        assert_eq!(action.mem.one_to_ten_us.count, 2);

        let action = context_stats
            .operations_context
            .iter()
            .find(|a| a.root == "b")
            .unwrap();
        assert_eq!(action.mem.one_to_ten_us.mean_time, 10.3);
        assert_eq!(action.mem.one_to_ten_us.count, 3);
        assert_eq!(action.add.one_to_ten_us.mean_time, 20.3);
        assert_eq!(action.add.one_to_ten_us.count, 4);
    }
}
