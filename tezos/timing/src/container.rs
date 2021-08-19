use std::{convert::TryInto, ops::Deref};

const INLINED_STRING_THRESHOLD: usize = 512;
const INLINED_HASH_LENGTH: usize = 32;

#[derive(Debug, PartialEq, Eq)]
pub enum InlinedHash {
    Inlined {
        array: [u8; INLINED_HASH_LENGTH],
    },
    Heap(Box<[u8]>)
}

impl InlinedHash {
    pub fn new(hash: &[u8]) -> Self {
        if hash.len() == INLINED_HASH_LENGTH {
            Self::Inlined {
                array: hash.try_into().unwrap()
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
            InlinedHash::Heap(heap) => &heap,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct InlinedContextHash(InlinedHash);
#[derive(Debug, PartialEq, Eq)]
pub struct InlinedOperationHash(InlinedHash);
#[derive(Debug, PartialEq, Eq)]
pub struct InlinedBlockHash(InlinedHash);

macro_rules! deref_inlined {
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

deref_inlined!(InlinedContextHash);
deref_inlined!(InlinedOperationHash);
deref_inlined!(InlinedBlockHash);

#[derive(Debug)]
pub enum InlinedString {
    Inlined {
        length: usize,
        array: [u8; INLINED_STRING_THRESHOLD],
    },
    Heap(Vec<u8>)
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
            },
            InlinedString::Heap(heap) => {
                heap.extend_from_slice(s.as_bytes())
            },
        }
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&*self).unwrap()
    }
}

impl Deref for InlinedString {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match self {
            InlinedString::Inlined { length, array } => &array[..*length],
            InlinedString::Heap(heap) => &heap,
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

            // let bytes = *s;
            println!("RES={:?}", &*s);
        }
    }
}
