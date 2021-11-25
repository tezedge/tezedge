// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::HashMap, convert::TryInto, path::PathBuf};

use chrono::{DateTime, NaiveDateTime, Utc};
use rusqlite::{named_params, Connection, OptionalExtension};
use serde::Serialize;

use crypto::hash::BlockHash;
use tezos_timing::{
    hash_to_string, Protocol, QueryData, QueryStats, QueryStatsWithRange, RangeStats, FILENAME_DB,
    STATS_ALL_PROTOCOL,
};

use crate::helpers::RpcServiceError;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockStats {
    queries_count: usize,
    date: Option<DateTime<Utc>>,
    duration_millis: Option<u64>,
    tezedge_checkout_context_time: Option<f64>,
    tezedge_commit_context_time: Option<f64>,
    irmin_checkout_context_time: Option<f64>,
    irmin_commit_context_time: Option<f64>,
    operations_context: Vec<QueryStats>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContextStats {
    commit_context: RangeStats,
    checkout_context: RangeStats,
    operations_context: Vec<QueryStatsWithRange>,
}

pub(crate) fn make_block_stats(
    db_path: Option<&PathBuf>,
    block_hash: BlockHash,
) -> Result<Option<BlockStats>, RpcServiceError> {
    let db_path = match db_path {
        Some(db_path) => {
            let mut db_path = db_path.to_path_buf();
            db_path.push(FILENAME_DB);
            db_path
        }
        None => PathBuf::from(FILENAME_DB),
    };

    let sql = Connection::open(db_path).map_err(|e| RpcServiceError::UnexpectedError {
        reason: format!("Failed to open context stats db, reason: {}", e),
    })?;
    make_block_stats_impl(&sql, block_hash).map_err(|e| RpcServiceError::UnexpectedError {
        reason: format!("Failed to make block stats, reason: {}", e),
    })
}

pub(crate) fn make_context_stats(
    db_path: Option<&PathBuf>,
    context_name: &str,
    protocol: Option<&str>,
) -> Result<ContextStats, RpcServiceError> {
    let db_path = match db_path {
        Some(db_path) => {
            let mut db_path = db_path.to_path_buf();
            db_path.push(FILENAME_DB);
            db_path
        }
        None => PathBuf::from(FILENAME_DB),
    };

    let protocol = match protocol {
        Some(protocol) => Some(Protocol::from_str(protocol).ok_or(
            RpcServiceError::InvalidParameters {
                reason: format!("Invalid 'protocol': {:?}", protocol),
            },
        )?),
        None => None,
    };

    let sql = Connection::open(db_path).map_err(|e| RpcServiceError::UnexpectedError {
        reason: format!("Failed to open context stats db, reason: {}", e),
    })?;
    make_context_stats_impl(&sql, context_name, protocol).map_err(|e| {
        RpcServiceError::UnexpectedError {
            reason: format!("Failed to make context stats, reason: {}", e),
        }
    })
}

fn make_context_stats_impl(
    sql: &Connection,
    context_name: &str,
    protocol: Option<Protocol>,
) -> Result<ContextStats, anyhow::Error> {
    let mut stmt = sql.prepare(
        "
    SELECT
      query_name,
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
      one_hundred_s_total_time,
      total_time,
      queries_count
    FROM
      global_query_stats
    WHERE
      context_name = :context_name AND protocol = :protocol
       ",
    )?;

    let mut rows = stmt.query(named_params! {
        ":context_name": context_name,
        ":protocol": protocol.as_ref().map(|p| p.as_str()).unwrap_or(STATS_ALL_PROTOCOL),
    })?;

    let mut map: HashMap<String, QueryStatsWithRange> = HashMap::default();
    let mut commit_stats = RangeStats::default();
    let mut checkout_stats = RangeStats::default();

    while let Some(row) = rows.next()? {
        let query_name = match row.get_ref(0)?.as_str() {
            Ok(name) if !name.is_empty() => name,
            _ => continue,
        };

        let root = match row.get_ref(1)?.as_str() {
            Ok(root) if !root.is_empty() => root,
            _ => continue,
        };

        let mut query_stats = match query_name {
            "commit" => &mut commit_stats,
            "checkout" => &mut checkout_stats,
            _ => {
                let entry = match map.get_mut(root) {
                    Some(entry) => entry,
                    None => {
                        let mut stats = QueryStatsWithRange::default();
                        stats.root = root.to_string();
                        map.insert(root.to_string(), stats);
                        map.get_mut(root).unwrap()
                    }
                };
                match query_name {
                    "mem" => &mut entry.mem,
                    "mem_tree" => &mut entry.mem_tree,
                    "find" => &mut entry.find,
                    "find_tree" => &mut entry.find_tree,
                    "add" => &mut entry.add,
                    "add_tree" => &mut entry.add_tree,
                    "remove" => &mut entry.remove,
                    _ => continue,
                }
            }
        };

        query_stats.one_to_ten_us.count = row.get(2)?;
        query_stats.one_to_ten_us.mean_time = row.get(3)?;
        query_stats.one_to_ten_us.max_time = row.get(4)?;
        query_stats.one_to_ten_us.total_time = row.get(5)?;
        query_stats.ten_to_one_hundred_us.count = row.get(6)?;
        query_stats.ten_to_one_hundred_us.mean_time = row.get(7)?;
        query_stats.ten_to_one_hundred_us.max_time = row.get(8)?;
        query_stats.ten_to_one_hundred_us.total_time = row.get(9)?;
        query_stats.one_hundred_us_to_one_ms.count = row.get(10)?;
        query_stats.one_hundred_us_to_one_ms.mean_time = row.get(11)?;
        query_stats.one_hundred_us_to_one_ms.max_time = row.get(12)?;
        query_stats.one_hundred_us_to_one_ms.total_time = row.get(13)?;
        query_stats.one_to_ten_ms.count = row.get(14)?;
        query_stats.one_to_ten_ms.mean_time = row.get(15)?;
        query_stats.one_to_ten_ms.max_time = row.get(16)?;
        query_stats.one_to_ten_ms.total_time = row.get(17)?;
        query_stats.ten_to_one_hundred_ms.count = row.get(18)?;
        query_stats.ten_to_one_hundred_ms.mean_time = row.get(19)?;
        query_stats.ten_to_one_hundred_ms.max_time = row.get(20)?;
        query_stats.ten_to_one_hundred_ms.total_time = row.get(21)?;
        query_stats.one_hundred_ms_to_one_s.count = row.get(22)?;
        query_stats.one_hundred_ms_to_one_s.mean_time = row.get(23)?;
        query_stats.one_hundred_ms_to_one_s.max_time = row.get(24)?;
        query_stats.one_hundred_ms_to_one_s.total_time = row.get(25)?;
        query_stats.one_to_ten_s.count = row.get(26)?;
        query_stats.one_to_ten_s.mean_time = row.get(27)?;
        query_stats.one_to_ten_s.max_time = row.get(28)?;
        query_stats.one_to_ten_s.total_time = row.get(29)?;
        query_stats.ten_to_one_hundred_s.count = row.get(30)?;
        query_stats.ten_to_one_hundred_s.mean_time = row.get(31)?;
        query_stats.ten_to_one_hundred_s.max_time = row.get(32)?;
        query_stats.ten_to_one_hundred_s.total_time = row.get(33)?;
        query_stats.one_hundred_s.count = row.get(34)?;
        query_stats.one_hundred_s.mean_time = row.get(35)?;
        query_stats.one_hundred_s.max_time = row.get(36)?;
        query_stats.one_hundred_s.total_time = row.get(37)?;
        query_stats.total_time = row.get(38)?;
        query_stats.queries_count = row.get(39)?;
    }

    let mut operations_context: Vec<_> = map
        .into_iter()
        .map(|(_, mut v)| {
            v.compute_total();
            v
        })
        .collect();

    operations_context.sort_by(|a, b| a.root.cmp(&b.root));

    Ok(ContextStats {
        operations_context,
        commit_context: commit_stats,
        checkout_context: checkout_stats,
    })
}

fn make_block_stats_impl(
    sql: &Connection,
    block_hash: BlockHash,
) -> Result<Option<BlockStats>, anyhow::Error> {
    let block_hash = hash_to_string(block_hash.as_ref());

    let (
        block_id,
        queries_count,
        tezedge_checkout_time,
        tezedge_commit_time,
        irmin_checkout_time,
        irmin_commit_time,
        duration_millis,
        timestamp_secs,
        timestamp_nanos,
    ) = match sql
        .query_row(
            "
    SELECT
      id,
      queries_count,
      checkout_time_tezedge,
      commit_time_tezedge,
      checkout_time_irmin,
      commit_time_irmin,
      duration_millis,
      timestamp_secs,
      timestamp_nanos
    FROM
      blocks
    WHERE
      hash = ?1;
        ",
            [block_hash],
            |row| {
                Ok((
                    row.get::<_, usize>(0)?,
                    row.get::<_, usize>(1)?,
                    row.get::<_, Option<f64>>(2)?,
                    row.get::<_, Option<f64>>(3)?,
                    row.get::<_, Option<f64>>(4)?,
                    row.get::<_, Option<f64>>(5)?,
                    row.get::<_, Option<u64>>(6)?,
                    row.get::<_, Option<u64>>(7)?,
                    row.get::<_, Option<u32>>(8)?,
                ))
            },
        )
        .optional()?
    {
        Some(row) => row,
        None => return Ok(None),
    };

    let mut stmt = sql.prepare(
        "
        SELECT
          root,
          tezedge_count,
          tezedge_mean_time,
          tezedge_max_time,
          tezedge_total_time,
          irmin_mean_time,
          irmin_max_time,
          irmin_total_time,
          tezedge_mem_time,
          tezedge_add_time,
          tezedge_add_tree_time,
          tezedge_find_time,
          tezedge_find_tree_time,
          tezedge_remove_time,
          irmin_mem_time,
          irmin_add_time,
          irmin_add_tree_time,
          irmin_find_time,
          irmin_find_tree_time,
          irmin_remove_time,
          irmin_count,
          tezedge_mem_tree_time,
          irmin_mem_tree_time
        FROM
          block_query_stats
        WHERE
          block_id = ?;
        ",
    )?;

    let mut rows = match stmt.query([block_id]).optional()? {
        Some(rows) => rows,
        None => return Ok(None),
    };

    let mut map: HashMap<String, QueryStats> = HashMap::default();

    while let Some(row) = rows.next()? {
        let root: String = match row.get(0) {
            Ok(root) => root,
            _ => continue,
        };

        let query_stats = QueryStats {
            data: QueryData {
                root: root.clone(),
                tezedge_count: row.get(1)?,
                irmin_count: row.get(20)?,
                tezedge_mean_time: row.get(2)?,
                tezedge_max_time: row.get(3)?,
                tezedge_total_time: row.get(4)?,
                irmin_mean_time: row.get(5)?,
                irmin_max_time: row.get(6)?,
                irmin_total_time: row.get(7)?,
            },
            tezedge_mem: row.get(8)?,
            tezedge_add: row.get(9)?,
            tezedge_add_tree: row.get(10)?,
            tezedge_find: row.get(11)?,
            tezedge_find_tree: row.get(12)?,
            tezedge_remove: row.get(13)?,
            irmin_mem: row.get(14)?,
            irmin_add: row.get(15)?,
            irmin_add_tree: row.get(16)?,
            irmin_find: row.get(17)?,
            irmin_find_tree: row.get(18)?,
            irmin_remove: row.get(19)?,
            tezedge_mem_tree: row.get(21)?,
            irmin_mem_tree: row.get(22)?,
        };

        map.insert(root, query_stats);
    }

    let mut operations_context: Vec<_> = map.into_iter().map(|(_, v)| v).collect();
    operations_context.sort_by(|a, b| a.data.root.cmp(&b.data.root));

    let mut date: Option<DateTime<Utc>> = None;
    if let (Some(secs), Some(nanos)) = (timestamp_secs, timestamp_nanos) {
        let secs = secs.try_into().unwrap_or(i64::MAX);
        let naive_date = NaiveDateTime::from_timestamp(secs, nanos);
        date = Some(DateTime::from_utc(naive_date, Utc));
    };

    Ok(Some(BlockStats {
        queries_count,
        tezedge_checkout_context_time: tezedge_checkout_time,
        tezedge_commit_context_time: tezedge_commit_time,
        irmin_checkout_context_time: irmin_checkout_time,
        irmin_commit_context_time: irmin_commit_time,
        operations_context,
        date,
        duration_millis,
    }))
}

#[cfg(test)]
mod tests {
    use rusqlite::Batch;

    use crypto::hash::HashTrait;

    use super::*;

    macro_rules! assert_float_eq {
        ($l:expr, $r:expr) => {{
            assert!(($l - $r).abs() < f64::EPSILON);
        }};
    }

    #[test]
    fn test_read_db() {
        let sql = Connection::open_in_memory().unwrap();

        let block_hash = BlockHash::try_from_bytes(&[1; 32]).unwrap();
        let block_hash_str = hash_to_string(block_hash.as_ref());

        let schema = include_str!("../../../tezos/timing/src/schema_stats.sql");
        let mut batch = Batch::new(&sql, schema);
        while let Some(mut stmt) = batch.next().unwrap() {
            stmt.execute([]).unwrap();
        }

        sql.execute(
            "
            INSERT INTO blocks
               (id, hash, queries_count, checkout_time_tezedge, commit_time_tezedge, checkout_time_irmin, commit_time_irmin, duration_millis)
            VALUES
               (1, ?1, 4, 10.0, 11.0, 12.0, 13.0, 101);",
            [block_hash_str],
        )
            .unwrap();

        sql.execute(
            "
        INSERT INTO block_query_stats
          (root, block_id, tezedge_count, tezedge_mean_time, tezedge_max_time, tezedge_total_time,
           tezedge_mem_time, tezedge_find_time, tezedge_find_tree_time, tezedge_add_time, tezedge_add_tree_time, tezedge_remove_time,
           irmin_mean_time, irmin_max_time, irmin_total_time,
           irmin_mem_time, irmin_find_time, irmin_find_tree_time, irmin_add_time, irmin_add_tree_time, irmin_remove_time,
           tezedge_mem_tree_time, irmin_mem_tree_time)
        VALUES
          ('a', 1, 40, 100.5, 3.0, 4.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
          ('m', 1, 400, 100.6, 30.0, 40.0, 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
            ",
            [],
        )
            .unwrap();

        let block_stats = make_block_stats_impl(&sql, block_hash).unwrap().unwrap();

        assert_eq!(block_stats.queries_count, 4);
        assert_float_eq!(block_stats.tezedge_checkout_context_time.unwrap(), 10.0);
        assert_float_eq!(block_stats.tezedge_commit_context_time.unwrap(), 11.0);
        assert_eq!(block_stats.operations_context.len(), 2);

        let query = block_stats
            .operations_context
            .iter()
            .find(|a| a.data.root == "a")
            .unwrap();
        assert_eq!(query.data.root, "a");
        assert_float_eq!(query.data.tezedge_mean_time, 100.5);
        assert_float_eq!(query.tezedge_add, 1.4);
        assert_float_eq!(query.tezedge_find_tree, 1.3);

        sql.execute(
            "
        INSERT INTO global_query_stats
          (context_name, protocol, query_name, root, one_to_ten_us_count, one_to_ten_us_mean_time,
           one_to_ten_us_max_time, one_to_ten_us_total_time, queries_count, total_time)
        VALUES
          ('tezedge', '_all_', 'mem', 'a', 2, 1.3, 1.4, 1.5, 1, 2.0),
          ('tezedge', '_all_', 'mem', 'b', 3, 10.3, 10.4, 10.5, 1, 2.0),
          ('tezedge', '_all_', 'add', 'b', 4, 20.3, 20.4, 20.5, 1, 2.0),
          ('tezedge', '_all_', 'commit', 'commit', 4, 30.3, 30.4, 30.5, 1, 2.0),
          ('tezedge', '_all_', 'checkout', 'checkout', 5, 40.3, 40.4, 40.5, 1, 2.0),
          ('tezedge', 'carthage', 'mem', 'a', 2, 1.3, 1.4, 1.5, 1, 2.0),
          ('tezedge', 'carthage', 'mem', 'b', 3, 10.3, 10.4, 10.5, 1, 2.0),
          ('tezedge', 'carthage', 'add', 'b', 4, 20.3, 20.4, 20.5, 1, 2.0),
          ('tezedge', 'carthage', 'commit', 'commit', 4, 30.3, 30.4, 30.5, 1, 2.0),
          ('tezedge', 'carthage', 'checkout', 'checkout', 5, 40.3, 40.4, 40.5, 1, 2.0);
            ",
            [],
        )
        .unwrap();

        for protocol in [None, Some(Protocol::Carthage)] {
            let context_stats = make_context_stats_impl(&sql, "tezedge", protocol).unwrap();

            assert_eq!(context_stats.operations_context.len(), 2);
            assert_float_eq!(context_stats.commit_context.one_to_ten_us.mean_time, 30.3);
            assert_eq!(context_stats.commit_context.queries_count, 1);
            assert_float_eq!(context_stats.checkout_context.one_to_ten_us.mean_time, 40.3);

            let query = context_stats
                .operations_context
                .iter()
                .find(|a| a.root == "a")
                .unwrap();
            assert_float_eq!(query.mem.one_to_ten_us.mean_time, 1.3);
            assert_eq!(query.mem.one_to_ten_us.count, 2);
            assert_float_eq!(query.total_time, 2.0);
            assert_float_eq!(query.mem.total_time, 2.0);

            let query = context_stats
                .operations_context
                .iter()
                .find(|a| a.root == "b")
                .unwrap();
            assert_float_eq!(query.mem.one_to_ten_us.mean_time, 10.3);
            assert_eq!(query.mem.one_to_ten_us.count, 3);
            assert_float_eq!(query.add.one_to_ten_us.mean_time, 20.3);
            assert_eq!(query.add.one_to_ten_us.count, 4);
        }

        let block_hash = BlockHash::try_from_bytes(&[32; 32]).unwrap();
        let block_stats = make_block_stats_impl(&sql, block_hash).unwrap();
        assert!(block_stats.is_none());
    }
}
