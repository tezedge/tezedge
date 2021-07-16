# Commit Log

Commit log stores sequential data its used in Tezedge to store block headers, commit log stores data in a single file, commit log uses `zstd` for compression, compression is disabled by default, can be enabled when initializing commit log `CommitLog::new(path, true)` , the second argument of the initialization function enables or disables compression for commit log.

## Methods

`append_msg` : Appends data to the commit log it take payload : `&[u8]` and returns the offset: `u64` of the data in the file and length : `usize` of the data in bytes or Commit Log error.

**Method signature:**

```rust
fn append_msg<B: AsRef<[u8]>>(&mut self,payload: B) -> Result<(u64,usize),CommitLogError>
```

`read` : Takes offset : `u64` and buffer_size: `usize`  returns the data: `Vec<u8>` or Commit Log error

**Method signature:**

```rust
fn read(&self, offset: u64, buf_size: usize) -> Result<Vec<u8>, CommitLogError>
```

## Usage

```rust
fn generate_random_data(
        data_size: usize,
        min_message_size: usize,
        max_message_size: usize,
    ) -> Vec<Vec<u8>>{..}
```

`generate_random_data` function generates random data with length(in bytes) between `min_message_size` and `max_message_size`

```rust
type ByteLimit = usize;
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Location(pub u64, pub ByteLimit);
```

```rust
let mut commit_log = CommitLog::new(new_commit_log_dir, false).unwrap();
let kv_store = sled::Config::new().temporary(true).open();
for (index,msg) in messages {
    let out = commit_log.append_msg(msg).unwrap();
    let location = Location(out.0, out.1);
    kv_store.insert(index,bincode::serialize(&location).unwrap());
}
```

## Source Code:

[https://github.com/tezedge/tezedge/tree/develop/storage/src/commit_log](https://github.com/tezedge/tezedge/tree/develop/storage/src/commit_log)

## Tests:

[https://github.com/tezedge/tezedge/blob/736391f63e53fc250953d4b31c6f693443eb8329/storage/src/commit_log/mod.rs#L388](https://github.com/tezedge/tezedge/blob/736391f63e53fc250953d4b31c6f693443eb8329/storage/src/commit_log/mod.rs#L388)

### How to Run Test

1. **Download TezEdge source code.**

```
# Open shell, type this code into the command line and then press Enter:
git clone https://github.com/tezedge/tezedge
cd tezedge
git checkout develop
```

2. Run.

```rust
cargo test --package storage --lib commit_log::tests::compare_with_old_log -- --exact
```