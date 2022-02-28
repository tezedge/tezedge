// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod common;

use crate::edgekv::EdgeKV;

use serial_test::serial;
//const N_THREADS: usize = 4;
const N_PER_THREAD: usize = 10;
//const N: usize = N_THREADS * N_PER_THREAD;
// NB N should be multiple of N_THREADS
//const SPACE: usize = N;
#[allow(dead_code)]
const INTENSITY: usize = 10;

//fn kv(i: usize) -> Vec<u8> {
//    let i = i % SPACE;
//    let k = [(i >> 16) as u8, (i >> 8) as u8, i as u8];
//    k.to_vec()
//}

fn clean_up(dir: &str) {
    fs_extra::dir::remove(format!("./testdir/{}", dir)).ok();
}

#[test]
#[serial]
fn monotonic_inserts() {
    clean_up("_test_monotonic_inserts");
    let db = EdgeKV::open("./testdir/_test_monotonic_inserts").unwrap();

    for len in [1_usize, 16, 32, 1024].iter() {
        for i in 0_usize..*len {
            let mut k = vec![];
            for c in 0_usize..i {
                k.push((c % 256) as u8);
            }
            db.put(k, vec![]).unwrap();
        }

        let count = db.iter().count();
        assert_eq!(count, *len as usize);

        let count2 = db.iter().rev().count();
        assert_eq!(count2, *len as usize);

        db.clear().unwrap();
        //clean_up();
    }

    for len in [1_usize, 16, 32, 1024].iter() {
        for i in (0_usize..*len).rev() {
            let mut k = vec![];
            for c in (0_usize..i).rev() {
                k.push((c % 256) as u8);
            }
            db.put(k, vec![]).unwrap();
        }

        let count3 = db.iter().count();
        assert_eq!(count3, *len as usize);

        let count4 = db.iter().rev().count();
        assert_eq!(count4, *len as usize);

        db.clear().unwrap();
    }
}

#[test]
#[serial]
fn get_set() {
    clean_up("_test_monotonic_get_set");
    {
        let db = EdgeKV::open("./testdir/_test_monotonic_get_set").unwrap();
        //let column = "hello";
        db.put(vec![2, 3, 4], vec![12, 23, 45]).unwrap();
        db.put(vec![22, 3, 4], vec![12, 23, 45]).unwrap();
        db.put(vec![2, 36, 4], vec![12, 23, 45]).unwrap();
        db.put(vec![2, 3, 4], vec![12, 23, 45]).unwrap();
        db.put(vec![12, 3, 42], vec![12, 23, 45]).unwrap();
        db.put(vec![2, 9, 4], vec![2, 9, 4]).unwrap();
    }
    {
        let db = EdgeKV::open("./testdir/_test_monotonic_get_set").unwrap();
        println!("{:?}", db.get(&vec![12, 3, 42]));
        println!("{:?}", db.get(&vec![12, 3, 42]));
    }
}

#[test]
#[serial]
fn prefix_test() {
    clean_up("_test_monotonic_prefix");
    {
        let db = EdgeKV::open("./testdir/_test_monotonic_prefix").unwrap();
        //let column = "hello";
        db.put(vec![2, 3, 4], vec![12, 23, 45]).unwrap();
        db.sync_all().unwrap();
        db.put(vec![2, 36, 4], vec![12, 23, 45]).unwrap();
        db.sync_all().unwrap();
        db.put(vec![3, 36, 4], vec![12, 23, 45]).unwrap();
        db.put(vec![3, 2, 4], vec![12, 23, 45]).unwrap();
        db.sync_all().unwrap();
        db.put(vec![2, 5, 4], vec![12, 23, 45]).unwrap();
        db.merge(concatenate_merge, vec![2, 5, 4], vec![1]).unwrap();
        db.merge(concatenate_merge, vec![2, 3, 4], vec![2]).unwrap();
        db.sync_all().unwrap();
        db.put(vec![2, 9, 4], vec![2, 9, 4]).unwrap();
        db.sync_all().unwrap();
    }
    {
        let db = EdgeKV::open("./testdir/_test_monotonic_prefix").unwrap();
        for res in db.prefix(&vec![2]) {
            println!("{:?}", res);
        }

        println!("...................................................................");
        for res in db.prefix(&vec![3_u8]) {
            println!("{:?}", res);
        }
        println!("...................................................................");
        for res in db.prefix(&vec![2]) {
            println!("{:?}", res);
        }

        println!("...................................................................");
        println!("{:?}", db.prefix(&vec![2]).count());
    }
}

#[test]
#[serial]
fn tree_big_keys_iterator() {
    clean_up("_test_tree_big_keys_iterator");
    fn kv(i: usize) -> Vec<u8> {
        let k = [(i >> 16) as u8, (i >> 8) as u8, i as u8];

        let mut base = vec![0; u8::max_value() as usize];
        base.extend_from_slice(&k);
        base
    }

    let t = EdgeKV::open("./testdir/_test_tree_big_keys_iterator").unwrap();
    for i in 0..N_PER_THREAD {
        let k = kv(i);
        t.put(k.clone(), k).unwrap();
        t.sync_all().unwrap();
    }

    for (i, (k, v)) in t.iter().map(|res| res.unwrap()).enumerate() {
        let should_be = kv(i);
        assert_eq!(should_be, &*k, "{}", t);
        assert_eq!(should_be, &*v);
    }

    for (i, (k, v)) in t.iter().map(|res| res.unwrap()).enumerate() {
        let should_be = kv(i);
        assert_eq!(should_be, &*k);
        assert_eq!(should_be, &*v);
    }

    let half_way = N_PER_THREAD / 2;
    let half_key = kv(half_way);
    let mut tree_scan = t.range(half_key.clone()..);
    let r1 = tree_scan.next().unwrap().unwrap();
    assert_eq!((r1.0, r1.1), (half_key.clone(), half_key));

    let first_key = kv(0);
    let mut tree_scan = t.range(first_key.clone()..);
    let r2 = tree_scan.next().unwrap().unwrap();
    assert_eq!((r2.0, r2.1), (first_key.clone(), first_key));

    let last_key = kv(N_PER_THREAD - 1);
    let mut tree_scan = t.range(last_key.clone()..);
    let r3 = tree_scan.next().unwrap().unwrap();
    assert_eq!((r3.0, r3.1), (last_key.clone(), last_key));
    assert!(tree_scan.next().is_none());
}

/*#[test]
#[serial]
fn concurrent_tree_ops_no() {
    clean_up("_test_concurrent_tree_ops");
    use std::thread;

    common::setup_logger();
    for i in 0..INTENSITY {
        debug!("beginning test {}", i);

        macro_rules! par {
            ($t:ident, $f:expr) => {
                let mut threads = vec![];
                for tn in 0..N_THREADS {
                    let tree = $t.clone();
                    let thread = thread::Builder::new()
                        .name(format!("t(thread: {} test: {})", tn, i))
                        .spawn(move || {
                            println!("thread: {}", tn);
                            for i in (tn * N_PER_THREAD)..((tn + 1) * N_PER_THREAD) {
                                let k = kv(i);
                                $f(&*tree, k);
                            }
                        })
                        .expect("should be able to spawn thread");
                    threads.push(thread);
                }
                while let Some(thread) = threads.pop() {
                    if let Err(e) = thread.join() {
                        panic!("thread failure: {:?}", e);
                    }
                }
            };
        }

        {
            debug!("========== initial sets test {} ==========", i);
            let t = Arc::new(EdgeKV::open("./testdir/_test_concurrent_tree_ops").unwrap());
            par! {t, |tree: &EdgeKV, k: Vec<u8>| {
                let res = tree.get(&k);
                if let Ok(None) = res {
                    assert!(true)
                }
                //assert_eq!(tree.get(&k), Ok(None));
                tree.put(k.clone(), k.clone()).expect("we should write successfully");
                    tree.sync_all().unwrap();
                assert_eq!(tree.get(&k).unwrap(), Some(k.clone().into()),
                    "failed to read key {:?} that we just wrote from tree {}",
                    k, tree);
            }};

            let n_scanned = t.iter().count();
            if n_scanned != N {
                warn!(
                    "WARNING: test {} only had {} keys present \
                 in the DB BEFORE restarting. expected {}",
                    i, n_scanned, N,
                );
            }
        }
        //    t.sync_all().unwrap();
        //println!("{} ")
        //drop(t);
        let t = Arc::new(EdgeKV::open("./testdir/_test_concurrent_tree_ops").unwrap());

        let n_scanned = t.iter().count();
        if n_scanned != N {
            warn!(
                "WARNING: test {} only had {} keys present \
                 in the DB AFTER restarting. expected {}",
                i, n_scanned, N,
            );
        }
        //println!("{}", t);
        debug!("========== reading sets in test {} ==========", i);
        par! {t, |tree: &EdgeKV, k: Vec<u8>| {
            if let Some(v) =  tree.get(&k).unwrap() {
                if v != k {
                    panic!("expected key {:?} not found", k);
                }
                //println!("{:?}",v);
            } else {
                panic!("could not read key {:?}, which we \
                       just wrote to tree {}", k, tree);
            }
        }};

        drop(t);
    }

    /*let t = Arc::new(EdgeKV::temp("./testdir/_test_concurrent_tree_ops").unwrap());

    debug!("========== deleting in test {} ==========", i);
    par! {t, |tree: &EdgeKV, k: Vec<u8>| {
        tree.delete(&k).unwrap();
    }};

    drop(t);
    let t = Arc::new(EdgeKV::temp("./testdir/_test_concurrent_tree_ops").unwrap());

    par! {t, |tree: &EdgeKV, k: Vec<u8>| {
        if let Ok(None) = tree.get(&k) {
            assert!(true)
        }
        else {
            assert!(false)
        }
        //assert_eq!(tree.get(&k), Ok(None));
    }};*/

    clean_up("_test_monotonic_inserts");
}
*/
fn concatenate_merge(
    _key: &[u8],                // the key being merged
    old_value: Option<Vec<u8>>, // the previous value, if one existed
    merged_bytes: &[u8],        // the new bytes being merged in
) -> Option<Vec<u8>> {
    // set the new value, return None to delete
    let mut ret = old_value.unwrap_or_else(|| vec![]);

    ret.extend_from_slice(merged_bytes);

    Some(ret)
}

#[test]
#[serial]
fn test_merge_operator() {
    clean_up("_test_merge_operator");
    let db = EdgeKV::open("./testdir/_test_merge_operator").unwrap();

    let k = b"k1";

    db.put(k.to_vec(), vec![0]).unwrap();
    db.merge(concatenate_merge, k.to_vec(), vec![1]).unwrap();
    db.merge(concatenate_merge, k.to_vec(), vec![2]).unwrap();
    db.sync_all().unwrap();
    assert_eq!(db.get(&k.to_vec()).unwrap().unwrap(), vec![0, 1, 2]);

    // Replace previously merged data. The merge function will not be called.
    db.put(k.to_vec(), vec![3]).unwrap();
    db.sync_all().unwrap();
    assert_eq!(db.get(&k.to_vec()).unwrap().unwrap(), vec![3]);

    // Merges on non-present values will cause the merge function to be called
    // with `old_value == None`. If the merge function returns something (which it
    // does, in this case) a new value will be inserted.
    //db.delete(&k.to_vec());
    //db.merge(concatenate_merge, k.to_vec(), vec![4]);
    //assert_eq!(db.get(&k.to_vec()).unwrap().unwrap(), vec![4]);
}
