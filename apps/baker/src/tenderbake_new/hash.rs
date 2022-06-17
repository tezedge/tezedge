// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub trait NoValue {
    fn no_value() -> Self;

    fn is_value(&self) -> bool;
}

#[cfg(feature = "testing-mock")]
mod hash_mock {
    use std::{collections::BTreeMap, fmt, sync::Mutex};

    use crypto::hash as orig;
    use serde::{Deserialize, Serialize};

    use super::NoValue;

    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    struct HashValue(u16);

    struct OracleItem<T> {
        dictionary: BTreeMap<T, HashValue>,
        backward: Vec<Option<T>>,
    }

    impl<T> Default for OracleItem<T> {
        fn default() -> Self {
            OracleItem {
                dictionary: BTreeMap::default(),
                backward: vec![],
            }
        }
    }

    impl<T> OracleItem<T>
    where
        T: Ord + Clone,
    {
        fn hash(&mut self, v: T) -> HashValue {
            if let Some(h) = self.dictionary.get(&v) {
                *h
            } else {
                let h = self.reserve(Some(v.clone()));
                self.dictionary.insert(v, h);
                h
            }
        }

        fn reserve(&mut self, real_value: Option<T>) -> HashValue {
            let c = self.backward.len();
            self.backward.push(real_value);
            HashValue(c as u16)
        }

        fn backward(&self, h: &HashValue) -> Option<&T> {
            self.backward.get(h.0 as usize).unwrap_or(&None).as_ref()
        }
    }

    lazy_static::lazy_static! {
        static ref BLOCK_HASH_ORACLE: Mutex<OracleItem<orig::BlockHash>> =
            Mutex::new(OracleItem::default());
        static ref BLOCK_PAYLOAD_HASH_ORACLE: Mutex<OracleItem<orig::BlockPayloadHash>> =
            Mutex::new(OracleItem::default());
        static ref OPERATION_HASH_ORACLE: Mutex<OracleItem<orig::OperationHash>> =
            Mutex::new(OracleItem::default());
        static ref ID_HASH_ORACLE: Mutex<OracleItem<orig::ContractTz1Hash>> =
            Mutex::new(OracleItem::default());
        static ref PROTOCOL_HASH_ORACLE: Mutex<OracleItem<orig::ProtocolHash>> =
            Mutex::new(OracleItem::default());
    }

    macro_rules! define_hash_mock {
        ($name:ident, $oracle:ident) => {
            #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
            pub struct $name(HashValue);

            impl NoValue for $name {
                fn no_value() -> Self {
                    let h = $oracle.lock().expect("poisoned").reserve(None);
                    $name(h)
                }

                fn is_value(&self) -> bool {
                    $oracle
                        .lock()
                        .expect("poisoned")
                        .backward(&self.0)
                        .is_some()
                }
            }

            impl From<orig::$name> for $name {
                fn from(orig: orig::$name) -> Self {
                    $name($oracle.lock().expect("poisoned").hash(orig))
                }
            }

            impl From<$name> for orig::$name {
                fn from(h: $name) -> Self {
                    $oracle
                        .lock()
                        .expect("poisoned")
                        .backward(&h.0)
                        .expect("hash is mock")
                        .clone()
                }
            }

            impl $name {
                pub fn from_base58_check(
                    data: &str,
                ) -> Result<Self, crypto::base58::FromBase58CheckError> {
                    orig::$name::from_base58_check(data).map(From::from)
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    match $oracle.lock().expect("poisoned").backward(&self.0) {
                        Some(v) => v.fmt(f),
                        None => write!(f, "mocked hash {}", self.0 .0),
                    }
                }
            }

            impl Serialize for $name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: serde::Serializer,
                {
                    self.to_string().serialize(serializer)
                }
            }

            impl<'de> Deserialize<'de> for $name {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: serde::Deserializer<'de>,
                {
                    let s = String::deserialize(deserializer)?;
                    Self::from_base58_check(&s).map_err(serde::de::Error::custom)
                }
            }
        };
    }

    define_hash_mock!(BlockHash, BLOCK_HASH_ORACLE);
    define_hash_mock!(BlockPayloadHash, BLOCK_PAYLOAD_HASH_ORACLE);
    define_hash_mock!(OperationHash, OPERATION_HASH_ORACLE);
    define_hash_mock!(ContractTz1Hash, ID_HASH_ORACLE);
    define_hash_mock!(ProtocolHash, PROTOCOL_HASH_ORACLE);
}
#[cfg(feature = "testing-mock")]
pub use self::hash_mock::{
    BlockHash, BlockPayloadHash, ContractTz1Hash, OperationHash, ProtocolHash,
};

#[cfg(not(feature = "testing-mock"))]
pub use crypto::hash::{BlockHash, BlockPayloadHash, ContractTz1Hash, OperationHash, ProtocolHash};

#[cfg(not(feature = "testing-mock"))]
impl NoValue for BlockHash {
    fn no_value() -> Self {
        BlockHash(vec![0x55; 32])
    }

    fn is_value(&self) -> bool {
        self.0.as_slice() != [0x55; 32]
    }
}

#[cfg(not(feature = "testing-mock"))]
impl NoValue for BlockPayloadHash {
    fn no_value() -> Self {
        BlockPayloadHash(vec![0x55; 32])
    }

    fn is_value(&self) -> bool {
        self.0.as_slice() != [0x55; 32]
    }
}
