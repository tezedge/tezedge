# Multi-Backend Database

Multi-backend is an abstraction independent of the database api, this allows Tezedge node not have multiple databases configurable at runtime, currently available database backend options include

- `sled` : Pure rust LSM key value store
- `rocksdb` : Widely used LSM key value store from Facebook

## Usage

To run the node with a different database backend set the value for flag `--maindb-backend` to `rocksdb` or `sled`

Example:

```bash
./run.sh --network=florencenet --maindb-backend=sled
```

## How to add new database

API

```rust
pub trait TezedgeDatabaseBackendStore {
    fn put(&self, column: &'static str, key: &[u8], value: &[u8]) -> Result<(), Error>;
    fn delete(&self, column: &'static str, key: &[u8]) -> Result<(), Error>;
    fn merge(&self, column: &'static str, key: &[u8], value: &[u8]) -> Result<(), Error>;
    fn get(&self, column: &'static str, key: &[u8]) -> Result<Option<Vec<u8>>, Error>;
    fn contains(&self, column: &'static str, key: &[u8]) -> Result<bool, Error>;
    fn write_batch(
        &self,
        column: &'static str,
        batch: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> Result<(), Error>;
    fn flush(&self) -> Result<usize, Error>;

    fn find(
        &self,
        column: &'static str,
        mode: BackendIteratorMode,
        limit: Option<usize>,
        filter: Box<dyn Fn((&[u8], &[u8])) -> Result<bool, SchemaError>>,
    ) -> Result<Vec<(Box<[u8]>, Box<[u8]>)>, Error>;
    fn find_by_prefix(
        &self,
        column: &'static str,
        key: &Vec<u8>,
        max_key_len: usize,
        filter: Box<dyn Fn((&[u8], &[u8])) -> Result<bool, SchemaError>>,
    ) -> Result<Vec<(Box<[u8]>, Box<[u8]>)>, Error>;
}
```

Database can be abstracted by implementing `TezedgeDatabaseBackendStore`

Examples:

**Rocks DB**

[https://github.com/tezedge/tezedge/blob/develop/storage/src/database/rockdb_backend.rs](https://github.com/tezedge/tezedge/blob/develop/storage/src/database/rockdb_backend.rs)
**Sled**

[https://github.com/tezedge/tezedge/blob/develop/storage/src/database/sled_backend.rs](https://github.com/tezedge/tezedge/blob/develop/storage/src/database/sled_backend.rs)

Edit `TezedgeDatabaseBackendConfiguration` enum and

```rust
pub enum TezedgeDatabaseBackendConfiguration {
    Sled,
    RocksDB,
    EdgeKV,
	//Add new database backend
}
```

```rust
impl TezedgeDatabaseBackendConfiguration {
    ...

    pub fn supported_values(&self) -> Vec<&'static str> {
        match self {
            Self::Sled => vec!["sled"],
            Self::RocksDB => vec!["rocksdb"],
						//Example: Self::LevelDB => vec!["leveldb"]
        }
    }
}
```

```rust
pub fn new(backend_option: TezedgeDatabaseBackendOptions) -> Self {
   match backend_option {
      TezedgeDatabaseBackendOptions::SledDB(backend) => TezedgeDatabase {
             backend: Arc::new(backend),
         },
      TezedgeDatabaseBackendOptions::RocksDB(backend) => TezedgeDatabase {
         backend: Arc::new(backend),
      },
			// Add match for backend
   }
}

```

## Tests

[https://github.com/tezedge/tezedge/tree/develop/storage/tests](https://github.com/tezedge/tezedge/tree/develop/storage/tests)

## How to run tests

```
# Open shell, type this code into the command line and then press Enter:
git clone https://github.com/mambisi/tezedge
cd tezedge
git checkout muti-backend-database-documentation
```

Run storage tests with `sled` as backend

```bash
cargo test storage --features maindb-backend-sled
```
Run storage tests with `rocksdb` as backend

```bash
cargo test storage --features maindb-backend-rocksdb
```

Rust storage tests with `edgekv` as backend

```bash
cargo test storage --features maindb-backend-edgekv
```