use crate::io_error_kind::IOErrorKind;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::io::ErrorKind;

pub fn serialize<S>(value: &ErrorKind, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    IOErrorKind::from(*value).serialize(s)
}

pub fn deserialize<'de, D>(d: D) -> Result<ErrorKind, D::Error>
where
    D: Deserializer<'de>,
{
    IOErrorKind::deserialize(d).map(|x| x.into())
}
