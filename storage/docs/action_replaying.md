# Action Replaying

## 1. Get an actions file

**Two possibilities:**
- see action recording documentation [here](action_recording.md)
- or use dedicated test to generate it
```
# build project
cargo build --release

# (from the project root) run test to generate action file
TARGET_ACTION_FILE=/tmp/test_action_file.data PROTOCOL_RUNNER=../target/release/protocol-runner cargo test --release -- --nocapture --ignored test_process_bootstrap_level1324_and_generate_action_file
```

Here we should have stored a new action file according to `TARGET_ACTION_FILE=/tmp/test_action_file.data`

## 2. Replay actions file - as `cargo run`

```
cargo run --release --bin context-actions-replayer -- --input /tmp/test_action_file.data --output /tmp/context_action_replayer --context-kv-store rocksdb
```

Check log processing:
```
dev@trace:~/tezedge$ cargo run --release --bin context-actions-replayer -- --input /tmp/test_action_file.data --output /tmp/context_action_replayer --context-kv-store rocksdb
    Finished release [optimized] target(s) in 0.26s
     Running `target/release/context-actions-replayer --input /tmp/test_action_file.data --output /tmp/context_action_replayer --context-kv-store rocksdb`
Mar 24 09:20:21.070 INFO Context actions replayer starts..., target_context_kv_store: RocksDb, output_kv_store_dir: /tmp/context_action_replayer, output_stats_file: /tmp/context_action_replayer/RocksDb.stats.txt, input_file: /tmp/test_action_file.data
Mar 24 09:20:21.115 INFO 1325 blocks found
Mar 24 09:20:21.116 DEBG progress 0.0754717% - cycle nr: 0 block nr 1 [8fcf233671b6a04fcf679d2a381c2544ea6c1ea29ba6157776ed8424bbcacb48] with 4 messages processed - 0 mb
Mar 24 09:20:25.424 DEBG progress 0.1509434% - cycle nr: 0 block nr 2 [47302477585709e79770f3c9c672b595f4499e29d718184a8fb739627a770ddd] with 36600 messages processed - 37 mb
Mar 24 09:20:26.381 DEBG progress 0.2264151% - cycle nr: 0 block nr 3 [e6324197ea27eff3fbb607e190d9a275e99b688aa56b2ee282566f928c1247da] with 13 messages processed - 37 mb
```

(see `output_stats_file: /tmp/context_action_replayer/RocksDb.stats.txt`)

(see `output_kv_store_dir: /tmp/context_action_replayer`)


## 3. Replay actions file - as standalone binary

```
# build binary
cargo build --release
cargo build --release --bin context-actions-replayer
```

```
# run binary
LD_LIBRARY_PATH=./tezos/sys/lib_tezos/artifacts ./target/release/context-actions-replayer --input /tmp/test_action_file.data --output /tmp/context_action_replayer --context-kv-store rocksdb
```

## 4. Result

- Ok result should be like this:
```
...
...
Mar 24 09:20:34.342 DEBG progress 99.6981132% - cycle nr: 0 block nr 1321 [0315d3478a76375cd8b491bb14c7586776b09fab59a1ea6e03e376172a42b7e3] with 49 messages processed - 105 mb
Mar 24 09:20:34.347 DEBG progress 99.7735849% - cycle nr: 0 block nr 1322 [22a2f33f4f4e8276c6846125deb567df2f247ecd6da9210c6ab66ab92b7296aa] with 49 messages processed - 106 mb
Mar 24 09:20:34.353 DEBG progress 99.8490566% - cycle nr: 0 block nr 1323 [0ab5051298e44bafda6bbe91e3d7b20cc0558ebb24b2d442a24f70e03621ad9a] with 49 messages processed - 106 mb
Mar 24 09:20:34.358 DEBG progress 99.9245283% - cycle nr: 0 block nr 1324 [0d5debe003599c10eab875e3a9e3f42f407d4f4aaa45132e1451a38dd4403b61] with 49 messages processed - 106 mb
Mar 24 09:20:34.363 DEBG progress 100.0000000% - cycle nr: 0 block nr 1325 [edc03d23aeaef497cd81b42e5f8e1dbf077174462c267ebe7c5099e78fc9cb01] with 49 messages processed - 106 mb
dev@trace:~/tezedge$
```
- You can check also detailed generated statistics here:
```
output_stats_file: /tmp/context_action_replayer/RocksDb.stats.txt
```
- You can now exlorer generated context storage from actions placed here:
```
output_kv_store_dir: /tmp/context_action_replayer

e.g.:
/tmp/context_action_replayer/replayed_context_rocksdb
```

## 5. Configuration
`--context-kv-store <kv-store>` - **inmem-gc, inmem, btree**
