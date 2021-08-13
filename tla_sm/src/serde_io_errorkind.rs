use serde::{Deserialize, Deserializer, Serializer};
use std::io::ErrorKind;

pub fn serialize<S>(value: &ErrorKind, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_u8(*value as u8)
}

pub fn deserialize<'de, D>(d: D) -> Result<ErrorKind, D::Error>
where
    D: Deserializer<'de>,
{
    let index: u8 = Deserialize::deserialize(d)?;

    Ok(match index {
        0 => ErrorKind::NotFound,
        1 => ErrorKind::PermissionDenied,
        2 => ErrorKind::ConnectionRefused,
        3 => ErrorKind::ConnectionReset,
        4 => ErrorKind::ConnectionAborted,
        5 => ErrorKind::NotConnected,
        6 => ErrorKind::AddrInUse,
        7 => ErrorKind::AddrNotAvailable,
        8 => ErrorKind::BrokenPipe,
        9 => ErrorKind::AlreadyExists,
        10 => ErrorKind::WouldBlock,
        11 => ErrorKind::InvalidInput,
        12 => ErrorKind::InvalidData,
        13 => ErrorKind::TimedOut,
        14 => ErrorKind::WriteZero,
        15 => ErrorKind::Interrupted,
        16 => ErrorKind::Other,
        17 => ErrorKind::UnexpectedEof,
        _ => return Err(serde::de::Error::custom("invalid ErrorKind index")),
    })
}
