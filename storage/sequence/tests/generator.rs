// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::path::Path;

use failure::Error;

use sequence::Sequences;
use storage::persistent::{KeyValueSchema, open_db};
use std::sync::Arc;

#[test]
fn generator_test_multiple_seq() -> Result<(), Error> {
    use rocksdb::{Options, DB};

    let path = "__sequence_multiseq";
    if Path::new(path).exists() {
        std::fs::remove_dir_all(path).unwrap();
    }

    {
        let db = open_db(path, vec![Sequences::descriptor()]).unwrap();
        let sequences = Sequences::new(Arc::new(db), 1);
        let gen_1 = sequences.generator("gen_1")?;
        let gen_2 = sequences.generator("gen_2")?;
        assert_eq!(0, gen_1.next()?);
        assert_eq!(1, gen_1.next()?);
        assert_eq!(2, gen_1.next()?);
        assert_eq!(0, gen_2.next()?);
        assert_eq!(3, gen_1.next()?);
        assert_eq!(1, gen_2.next()?);
    }
    Ok(assert!(DB::destroy(&Options::default(), path).is_ok()))
}

#[test]
fn generator_test_batch() -> Result<(), Error> {
    use rocksdb::{Options, DB};

    let path = "__sequence_batch";
    if Path::new(path).exists() {
        std::fs::remove_dir_all(path).unwrap();
    }

    {
        let db = open_db(path, vec![Sequences::descriptor()])?;
        let sequences = Sequences::new(Arc::new(db), 100);
        let gen = sequences.generator("gen")?;
        for i in 0..1_000_000 {
            assert_eq!(i, gen.next()?);
        }
    }
    Ok(assert!(DB::destroy(&Options::default(), path).is_ok()))
}

#[test]
fn generator_test_continuation_after_persist() -> Result<(), Error> {
    use rocksdb::{Options, DB};

    let path = "__sequence_continuation";
    if Path::new(path).exists() {
        std::fs::remove_dir_all(path).unwrap();
    }

    {
        let db = Arc::new(open_db(path, vec![Sequences::descriptor()])?);

        // First run
        {
            let sequences = Sequences::new(db.clone(), 10);
            let gen = sequences.generator("gen")?;
            for i in 0..7 {
                assert_eq!(i, gen.next()?, "First run failed");
            }
        }

        // Second run should continue from the number stored in a database, in this case 10
        {
            let sequences = Sequences::new(db.clone(), 10);
            let gen = sequences.generator("gen")?;
            for i in 0..=50 {
                assert_eq!(10 + i, gen.next()?, "Second run failed");
            }
        }

        // Third run should continue from the number stored in a database, in this case 70.
        // It's 70 because previous sequence ended at 60, so generator pre-allocated
        // another 10 sequence numbers, so 60 + 10 = 70. but none of those pre-allocated numbers
        // were used in the second run.
        {
            let sequences = Sequences::new(db.clone(), 10);
            let gen = sequences.generator("gen")?;
            for i in 0..5 {
                assert_eq!(70 + i, gen.next()?, "Third run failed");
            }
        }
    }
    Ok(assert!(DB::destroy(&Options::default(), path).is_ok()))
}