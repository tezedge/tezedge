use rocksdb::{DB, ReadOptions, WriteOptions};

use crate::persistent::schema::{Schema, SchemaError, SchemaIterator};

pub trait DatabaseWithScheme {
    fn put<S: Schema>(&self, key: &S::Key, value: &S::Value) -> Result<(), SchemaError>;

    fn get<S: Schema>(&self, key: &S::Key) -> Result<Option<S::Value>, SchemaError>;
}

impl DatabaseWithScheme for DB {
    fn put<S: Schema>(&self, key: &S::Key, value: &S::Value) -> Result<(), SchemaError> {
        let key = key.encode()?;
        let value = value.encode()?;
        let cf_handle = self.get_cf_handle(S::COLUMN_FAMILY_NAME)?;

        self.put_cf_opt(cf_handle, &key, &value, &default_write_options())
            .map_err(convert_rocksdb_err)
    }

    fn get<S: Schema>(&self, key: &S::Key) -> Result<Option<S::Value>, SchemaError> {
        let key = key.encode()?;
        let cf_handle = self.get_cf_handle(S::COLUMN_FAMILY_NAME)?;

        self.get_cf(cf_handle, &k)
            .map(|value| S::Value::decode(&value))
            .transpose()
    }
}

fn default_write_options() -> WriteOptions {
    let mut opts = WriteOptions::new();
    opts.set_sync(true);
    opts
}