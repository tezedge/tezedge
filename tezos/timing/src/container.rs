// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Implementation of containers where we try to avoid the cost of an allocation
//! when we receive hashes and strings in the rust timing hooks.

use std::{convert::TryInto, ops::Deref};

const INLINED_STRING_THRESHOLD: usize = 512;
const INLINED_HASH_LENGTH: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InlinedHash {
    Inlined { array: [u8; INLINED_HASH_LENGTH] },
    Heap(Box<[u8]>),
}

impl InlinedHash {
    pub fn new(hash: &[u8]) -> Self {
        if hash.len() == INLINED_HASH_LENGTH {
            Self::Inlined {
                array: hash.try_into().unwrap(),
            }
        } else {
            Self::Heap(Box::from(hash))
        }
    }
}

impl Deref for InlinedHash {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match self {
            InlinedHash::Inlined { array } => &array[..],
            InlinedHash::Heap(heap) => heap,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InlinedContextHash(InlinedHash);
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InlinedOperationHash(InlinedHash);
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InlinedBlockHash(InlinedHash);
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InlinedProtocolHash(InlinedHash);

macro_rules! inlined_hash_impl {
    ($hash:ident) => {
        impl Deref for $hash {
            type Target = [u8];

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl AsRef<[u8]> for $hash {
            fn as_ref(&self) -> &[u8] {
                self
            }
        }

        impl From<&[u8]> for $hash {
            fn from(slice: &[u8]) -> Self {
                Self(InlinedHash::new(slice))
            }
        }
    };
}

inlined_hash_impl!(InlinedContextHash);
inlined_hash_impl!(InlinedOperationHash);
inlined_hash_impl!(InlinedBlockHash);
inlined_hash_impl!(InlinedProtocolHash);

#[derive(Debug)]
pub enum InlinedString {
    Inlined {
        length: usize,
        array: [u8; INLINED_STRING_THRESHOLD],
    },
    Heap(Vec<u8>),
}

impl Default for InlinedString {
    fn default() -> Self {
        Self::new()
    }
}

impl InlinedString {
    pub fn new() -> Self {
        Self::Inlined {
            length: 0,
            array: [0; INLINED_STRING_THRESHOLD],
        }
    }

    pub fn push_str(&mut self, s: &str) {
        match self {
            InlinedString::Inlined { length, array } => {
                let new_length = *length + s.len();

                if new_length > INLINED_STRING_THRESHOLD {
                    let mut heap = Vec::with_capacity(new_length);
                    heap.extend_from_slice(&array[..*length]);
                    heap.extend_from_slice(s.as_bytes());
                    *self = Self::Heap(heap);
                } else {
                    array[*length..new_length].copy_from_slice(s.as_bytes());
                    *length = new_length;
                }
            }
            InlinedString::Heap(heap) => heap.extend_from_slice(s.as_bytes()),
        }
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&*self).unwrap()
    }
}

impl From<&[&str]> for InlinedString {
    fn from(list: &[&str]) -> Self {
        let mut inlined = Self::new();

        for s in list {
            inlined.push_str(s);
        }

        inlined
    }
}

impl Deref for InlinedString {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match self {
            InlinedString::Inlined { length, array } => &array[..*length],
            InlinedString::Heap(heap) => heap,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inlined_str() {
        let mut s = InlinedString::new();

        for i in 0..INLINED_STRING_THRESHOLD + 10 {
            s.push_str("a");
            let bytes = &*s;

            assert_eq!(bytes.len(), i + 1);
        }
    }
}
