// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Implementation of string interning used to implement hash-consing for context path fragments.
//! This avoids un-necessary duplication of strings, saving memory.

use std::{collections::hash_map::DefaultHasher, hash::Hasher};

use static_assertions::const_assert;
use tezos_timing::StringsMemoryUsage;

use crate::Map;

pub(crate) const STRING_INTERN_THRESHOLD: usize = 30;

const FULL_31_BITS: usize = 0x7FFFFFFF;
const FULL_5_BITS: usize = 0x1F;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StringId {
    /// | 1 bit  |  31 bits |
    /// |--------|----------|
    /// | is_big |  value   |
    ///
    /// Value not big:
    /// |        26 bits        | 5 bits |
    /// |-----------------------|--------|
    /// | offset in all_strings | length |
    ///
    /// Value big:
    /// |          31 bits           |
    /// |----------------------------|
    /// | index in BigString.offsets |
    bits: u32,
}

// The number of bits for the string length in the
// the bitfield is 5
const_assert!(STRING_INTERN_THRESHOLD < (1 << 5));

impl StringId {
    fn is_big(self) -> bool {
        (self.bits >> 31) != 0
    }

    fn get_big_index(self) -> usize {
        self.bits as usize & FULL_31_BITS
    }

    fn get_start_end(self) -> (usize, usize) {
        let start = (self.bits >> FULL_5_BITS.count_ones()) as usize;
        let length = self.bits as usize & FULL_5_BITS;

        (start, start + length)
    }
}

#[derive(Default)]
struct BigStrings {
    strings: String,
    offsets: Vec<(u32, u32)>,
}

impl BigStrings {
    fn push_str(&mut self, s: &str) -> u32 {
        let start = self.strings.len();
        self.strings.push_str(s);
        let end = self.strings.len();

        let index = self.offsets.len();
        self.offsets.push((start as u32, end as u32));

        index as u32
    }

    fn get_str(&self, index: usize) -> Option<&str> {
        let (start, end) = self.offsets.get(index).copied()?;
        self.strings.get(start as usize..end as usize)
    }

    fn clear(&mut self) {
        let cap = self.strings.capacity();

        if cap > 1_000_000 {
            let new_cap = (cap / 2).max(1_000_000);
            self.strings = String::with_capacity(new_cap);
        } else {
            self.strings.clear();
        }

        self.offsets.clear();
    }
}

#[derive(Default)]
pub struct StringInterner {
    /// `Map` of hash of the string to their `StringId`
    /// We don't use `HashMap<String, StringId>` because the map would
    /// keep a copy of the string
    string_to_offset: Map<u64, StringId>,
    /// Concatenation of all strings < STRING_INTERN_THRESHOLD.
    /// This is never cleared/deallocated
    all_strings: String,
    /// Concatenation of big strings. This is cleared/deallocated
    /// before every checkouts
    big_strings: BigStrings,
}

impl StringInterner {
    pub fn get_string_id(&mut self, s: &str) -> StringId {
        if s.len() >= STRING_INTERN_THRESHOLD {
            let index = self.big_strings.push_str(s);

            return StringId {
                bits: 1 << 31 | index,
            };
        }

        let mut hasher = DefaultHasher::new();
        hasher.write(s.as_bytes());
        let hashed = hasher.finish();

        if let Some(string_id) = self.string_to_offset.get(&hashed) {
            return *string_id;
        }

        let index: u32 = self.all_strings.len() as u32;
        let length: u32 = s.len() as u32;

        assert_eq!(index & !0x3FFFFFF, 0);

        self.all_strings.push_str(s);

        let string_id = StringId {
            bits: index << 5 | length,
        };

        self.string_to_offset.insert(hashed, string_id);

        debug_assert_eq!(s, self.get(string_id).unwrap());

        string_id
    }

    pub fn get(&self, string_id: StringId) -> Option<&str> {
        if string_id.is_big() {
            return self.big_strings.get_str(string_id.get_big_index());
        }

        let (start, end) = string_id.get_start_end();
        self.all_strings.get(start..end)
    }

    pub fn clear(&mut self) {
        self.big_strings.clear();
    }

    pub fn memory_usage(&self) -> StringsMemoryUsage {
        let all_strings_cap = self.all_strings.capacity();
        let big_strings_cap = self.big_strings.strings.capacity();

        StringsMemoryUsage {
            all_strings_map_cap: self.string_to_offset.capacity(),
            all_strings_map_len: self.string_to_offset.len(),
            all_strings_cap,
            all_strings_len: self.all_strings.len(),
            big_strings_cap,
            big_strings_len: self.big_strings.strings.len(),
            big_strings_map_cap: self.big_strings.offsets.capacity(),
            big_strings_map_len: self.big_strings.offsets.len(),
            total_bytes: all_strings_cap + big_strings_cap,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_interner() {
        let mut interner = StringInterner::default();

        let a = interner.get_string_id("a");
        let b = interner.get_string_id("a");

        assert_eq!(a, b);
        assert!(!a.is_big());
        assert_eq!(interner.get(a), Some("a"));
        assert_eq!(interner.get(a), interner.get(b));

        let long_str = std::iter::repeat("a")
            .take(STRING_INTERN_THRESHOLD)
            .collect::<String>();

        let a = interner.get_string_id(&long_str);
        let b = interner.get_string_id(&long_str);
        assert_ne!(a, b);
        assert!(a.is_big());
        assert_eq!(interner.get(a).unwrap(), long_str);
        assert_eq!(interner.get(b).unwrap(), long_str);
    }
}
