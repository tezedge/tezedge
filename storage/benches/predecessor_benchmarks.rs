use std::collections::HashSet;
use std::convert::TryInto;

use failure::Error;
use rand::Rng;

use crypto::hash::BlockHash;

use criterion::{criterion_group, criterion_main, Criterion};

use storage::tests_common::TmpStorage;
use storage::{BlockMetaStorage, BlockMetaStorageReader};
use storage::block_meta_storage::Meta;

/// Create and return a storage with [number_of_blocks] blocks and the last BlockHash in it
fn init_mocked_storage(number_of_blocks: usize) -> Result<(BlockMetaStorage, BlockHash), Error> {
    let tmp_storage = TmpStorage::create("__mocked_storage")?;
    let storage = BlockMetaStorage::new(tmp_storage.storage());
    let mut block_hash_set = HashSet::new();
    let mut rng = rand::thread_rng();

    let k = vec![0; 32];
    let v = Meta::new(false, Some(vec![0; 32]), 0, vec![44; 4]);

    block_hash_set.insert(k.clone());

    storage.put(&k, &v)?;
    assert!(storage.get(&k)?.is_some());

    // save for the iteration
    let mut predecessor = k.clone();

    // generate random block hashes, watch out for colissions
    let block_hashes: Vec<BlockHash> = (1..number_of_blocks).map(|_| {
        let mut random_hash: BlockHash = (0..32).map(|_| rng.gen_range(0, 255)).collect();
        // regenerate on collision
        while block_hash_set.contains(&random_hash) {
            random_hash = (0..32).map(|_| rng.gen_range(0, 255)).collect();
        }
        block_hash_set.insert(random_hash.clone());
        random_hash
    }).collect();

    // add them to the mocked storage
    for (idx, block_hash) in block_hashes.iter().enumerate() {
        let v = Meta::new(true, Some(predecessor.clone()), idx.try_into()?, vec![44; 4]);
        storage.put(&block_hash, &v)?;
        predecessor = block_hash.clone();
    }

    Ok((storage, predecessor))
}

fn find_block_at_distance_benchmark(c: &mut Criterion) {
    let (storage, last_block_hash) = init_mocked_storage(100_000).unwrap();

    c.bench_function("find_block_at_distance", |b| {
        b.iter(|| storage.find_block_at_distance(last_block_hash.clone(), 99_999))
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = find_block_at_distance_benchmark
}

criterion_main!(benches);