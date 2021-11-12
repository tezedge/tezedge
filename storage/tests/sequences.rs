// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Error;
use rocksdb::Cache;

use storage::database;
use storage::database::tezedge_database::{TezedgeDatabase, TezedgeDatabaseBackendOptions};
use storage::persistent::database::{open_kv, RocksDbKeyValueSchema};
use storage::persistent::sequence::Sequences;
use storage::persistent::DbConfiguration;
use storage::tests_common;

#[test]
fn generator_test_multiple_gen() -> Result<(), Error> {
    use rocksdb::{Options, DB};

    // logger
    let log_level = tests_common::log_level();
    let log = tests_common::create_logger(log_level);

    let path = out_dir_path("__sequence_multigen");
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }

    {
        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let backend = if cfg!(feature = "maindb-backend-rocksdb") {
            let db = open_kv(
                &path,
                vec![Sequences::descriptor(&cache)],
                &DbConfiguration::default(),
            )?;
            TezedgeDatabaseBackendOptions::RocksDB(
                database::rockdb_backend::RocksDBBackend::from_db(Arc::new(db))?,
            )
        } else if cfg!(feature = "maindb-backend-sled") {
            TezedgeDatabaseBackendOptions::SledDB(database::sled_backend::SledDBBackend::new(
                path.join("db"),
            )?)
        }
        else if cfg!(feature = "maindb-backend-edgekv") {
            TezedgeDatabaseBackendOptions::EdgeKV(database::edgekv_backend::EdgeKVBackend::new(
                path.join("db"),
                vec![
                    Sequences::name()
                ]
            )?)
        }else {
            let db = open_kv(
                &path,
                vec![Sequences::descriptor(&cache)],
                &DbConfiguration::default(),
            )?;
            TezedgeDatabaseBackendOptions::RocksDB(
                database::rockdb_backend::RocksDBBackend::from_db(Arc::new(db))?,
            )
        };
        let maindb = Arc::new(TezedgeDatabase::new(backend, log));

        let sequences = Sequences::new(maindb, 1);
        let gen_1 = sequences.generator("gen_1");
        let gen_2 = sequences.generator("gen_2");
        assert_eq!(0, gen_1.next()?);
        assert_eq!(1, gen_1.next()?);
        assert_eq!(2, gen_1.next()?);
        assert_eq!(0, gen_2.next()?);
        assert_eq!(3, gen_1.next()?);
        assert_eq!(1, gen_2.next()?);
    }
    assert!(DB::destroy(&Options::default(), &path).is_ok());
    Ok(())
}

#[test]
fn generator_test_cloned_gen() -> Result<(), Error> {
    use rocksdb::{Options, DB};

    // logger
    let log_level = tests_common::log_level();
    let log = tests_common::create_logger(log_level);

    let path = out_dir_path("__sequence_multiseq");
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }

    {
        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let backend = if cfg!(feature = "maindb-backend-rocksdb") {
            let db = open_kv(
                &path,
                vec![Sequences::descriptor(&cache)],
                &DbConfiguration::default(),
            )?;
            TezedgeDatabaseBackendOptions::RocksDB(
                database::rockdb_backend::RocksDBBackend::from_db(Arc::new(db))?,
            )
        } else if cfg!(feature = "maindb-backend-sled") {
            TezedgeDatabaseBackendOptions::SledDB(database::sled_backend::SledDBBackend::new(
                path.join("db"),
            )?)
        }
        else if cfg!(feature = "maindb-backend-edgekv") {
            TezedgeDatabaseBackendOptions::EdgeKV(database::edgekv_backend::EdgeKVBackend::new(
                path.join("db"),
                vec![
                    Sequences::name()
                ]
            )?)
        }else {
            let db = open_kv(
                &path,
                vec![Sequences::descriptor(&cache)],
                &DbConfiguration::default(),
            )?;
            TezedgeDatabaseBackendOptions::RocksDB(
                database::rockdb_backend::RocksDBBackend::from_db(Arc::new(db))?,
            )
        };
        let maindb = Arc::new(TezedgeDatabase::new(backend, log));
        let sequences = Sequences::new(maindb, 3);
        let gen_a = sequences.generator("gen");
        let gen_b = sequences.generator("gen");
        assert_eq!(0, gen_a.next()?);
        assert_eq!(1, gen_a.next()?);
        assert_eq!(2, gen_a.next()?);
        assert_eq!(3, gen_b.next()?);
        assert_eq!(4, gen_a.next()?);
        assert_eq!(5, gen_b.next()?);
        assert_eq!(6, gen_b.next()?);
        assert_eq!(7, gen_a.next()?);
    }
    assert!(DB::destroy(&Options::default(), &path).is_ok());
    Ok(())
}

#[test]
fn generator_test_batch() -> Result<(), Error> {
    use rocksdb::{Options, DB};

    // logger
    let log_level = tests_common::log_level();
    let log = tests_common::create_logger(log_level);

    let path = out_dir_path("__sequence_batch");
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }

    {
        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let backend = if cfg!(feature = "maindb-backend-rocksdb") {
            let db = open_kv(
                &path,
                vec![Sequences::descriptor(&cache)],
                &DbConfiguration::default(),
            )?;
            TezedgeDatabaseBackendOptions::RocksDB(
                database::rockdb_backend::RocksDBBackend::from_db(Arc::new(db))?,
            )
        } else if cfg!(feature = "maindb-backend-sled") {
            TezedgeDatabaseBackendOptions::SledDB(database::sled_backend::SledDBBackend::new(
                path.join("db"),
            )?)
        }
        else if cfg!(feature = "maindb-backend-edgekv") {
            TezedgeDatabaseBackendOptions::EdgeKV(database::edgekv_backend::EdgeKVBackend::new(
                path.join("db"),
                vec![
                    Sequences::name()
                ]
            )?)
        }else {
            let db = open_kv(
                &path,
                vec![Sequences::descriptor(&cache)],
                &DbConfiguration::default(),
            )?;
            TezedgeDatabaseBackendOptions::RocksDB(
                database::rockdb_backend::RocksDBBackend::from_db(Arc::new(db))?,
            )
        };

        let maindb = Arc::new(TezedgeDatabase::new(backend, log));
        let sequences = Sequences::new(maindb, 100);
        let gen = sequences.generator("gen");
        for i in 0..1_000_000 {
            assert_eq!(i, gen.next()?);
        }
    }
    assert!(DB::destroy(&Options::default(), &path).is_ok());
    Ok(())
}

#[test]
fn generator_test_continuation_after_persist() -> Result<(), Error> {
    use rocksdb::{Options, DB};

    // logger
    let log_level = tests_common::log_level();
    let log = tests_common::create_logger(log_level);

    let path = out_dir_path("__sequence_continuation");
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }

    {
        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let backend = if cfg!(feature = "maindb-backend-rocksdb") {
            let db = open_kv(
                &path,
                vec![Sequences::descriptor(&cache)],
                &DbConfiguration::default(),
            )?;
            TezedgeDatabaseBackendOptions::RocksDB(
                database::rockdb_backend::RocksDBBackend::from_db(Arc::new(db))?,
            )
        } else if cfg!(feature = "maindb-backend-sled") {
            TezedgeDatabaseBackendOptions::SledDB(database::sled_backend::SledDBBackend::new(
                path.join("db"),
            )?)
        }
        else if cfg!(feature = "maindb-backend-edgekv") {
            TezedgeDatabaseBackendOptions::EdgeKV(database::edgekv_backend::EdgeKVBackend::new(
                path.join("db"),
                vec![
                    Sequences::name()
                ]
            )?)
        }
        else {
            let db = open_kv(
                &path,
                vec![Sequences::descriptor(&cache)],
                &DbConfiguration::default(),
            )?;
            TezedgeDatabaseBackendOptions::RocksDB(
                database::rockdb_backend::RocksDBBackend::from_db(Arc::new(db))?,
            )
        };

        let maindb = Arc::new(TezedgeDatabase::new(backend, log));

        // First run
        {
            let sequences = Sequences::new(maindb.clone(), 10);
            let gen = sequences.generator("gen");
            for i in 0..7 {
                assert_eq!(i, gen.next()?, "First run failed");
            }
        }

        // Second run should continue from the number stored in a database, in this case 10
        {
            let sequences = Sequences::new(maindb.clone(), 10);
            let gen = sequences.generator("gen");
            for i in 0..=50 {
                assert_eq!(10 + i, gen.next()?, "Second run failed");
            }
        }

        // Third run should continue from the number stored in a database, in this case 70.
        // It's 70 because previous sequence ended at 60, so generator pre-allocated
        // another 10 sequence numbers, so 60 + 10 = 70. but none of those pre-allocated numbers
        // were used in the second run.
        {
            let sequences = Sequences::new(maindb, 10);
            let gen = sequences.generator("gen");
            for i in 0..5 {
                assert_eq!(70 + i, gen.next()?, "Third run failed");
            }
        }
    }
    assert!(DB::destroy(&Options::default(), &path).is_ok());
    Ok(())
}

fn out_dir_path(dir_name: &str) -> PathBuf {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined");
    Path::new(out_dir.as_str()).join(Path::new(dir_name))
}
