// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Implementation of string interning used to implement hash-consing for context path fragments.
//! This avoids un-necessary duplication of strings, saving memory.

use std::io::Read;
use std::{borrow::Cow, collections::hash_map::DefaultHasher, convert::TryInto, hash::Hasher};

use static_assertions::const_assert;
use tezos_timing::StringsMemoryUsage;

use crate::gc::SortedMap;
use crate::{
    chunks::{ChunkedString, ChunkedVec},
    persistent::file::{File, TAG_BIG_STRINGS, TAG_STRINGS},
    serialize::DeserializationError,
};

use super::storage::StorageError;

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
    pub fn is_big(self) -> bool {
        (self.bits >> 31) != 0
    }

    fn get_big_index(self) -> usize {
        self.bits as usize & FULL_31_BITS
    }

    pub fn get_start_end(self) -> (usize, usize) {
        let start = (self.bits >> FULL_5_BITS.count_ones()) as usize;
        let length = self.bits as usize & FULL_5_BITS;

        (start, start + length)
    }

    pub fn as_u32(self) -> u32 {
        self.bits
    }

    pub(crate) fn deserialize(string_id_bytes: [u8; 4]) -> Self {
        let bits = u32::from_le_bytes(string_id_bytes);
        Self { bits }
    }
}

pub struct SerializeStrings {
    /// Concatenation of all strings STRING_INTERN_THRESHOLD and above
    ///
    /// Example:
    /// ['a', 'b', 'c', 'd', 'e,]
    pub big_strings: Vec<u8>,
    /// Contains all strings below STRING_INTERN_THRESHOLD
    ///
    /// Format is [length, [string], length, [string], ..]
    /// Example:
    /// [3, 'a', 'b', 'c', 1, 'z', 4, 'd', 'c', 'b', 'a']
    pub strings: Vec<u8>,
}

#[derive(Clone, Default)]
struct BigStrings {
    hashes: SortedMap<u64, u32>,
    strings: ChunkedString<{ 64 * 1024 * 1024 }>, // ~67MB
    offsets: ChunkedVec<(u32, u32), { 128 * 1024 }>, // ~1MB
    to_serialize_index: usize,
    new_bytes_since_last_serialize: usize,
}

impl std::fmt::Debug for BigStrings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BigStrings")
            .field("hashes_bytes", &self.hashes.total_bytes())
            .field("strings_cap", &self.strings.capacity())
            .field("offsets_cap", &self.offsets.capacity())
            .field(
                "offsets_bytes",
                &(self.offsets.capacity() * std::mem::size_of::<(u32, u32)>()),
            )
            .field("to_serialize_index", &self.to_serialize_index)
            .finish()
    }
}

impl PartialEq for BigStrings {
    fn eq(&self, other: &Self) -> bool {
        self.strings.len() == other.strings.len()
    }
}

impl Eq for BigStrings {}

impl BigStrings {
    fn push_str(&mut self, s: &str) -> u32 {
        let mut hasher = DefaultHasher::new();
        hasher.write(s.as_bytes());
        let hashed = hasher.finish();

        if let Some(offset) = self.hashes.get(&hashed) {
            return *offset;
        }

        let (start, length) = self.strings.push_str(s);
        let end = start + length;

        let index = self.offsets.push((start as u32, end as u32)) as u32;

        self.hashes.insert(hashed, index);
        self.new_bytes_since_last_serialize += s.len();

        index
    }

    fn get_str(&self, index: usize) -> Option<Cow<str>> {
        let (start, end) = self.offsets.get(index).copied()?;
        self.strings.get(start as usize..end as usize)
    }

    fn extend_from(&mut self, other: &Self) {
        if self == other {
            return;
        }

        debug_assert!(self.strings.len() < other.strings.len());
        // Append the missing chunk into Self
        self.strings.extend_from(&other.strings);
        debug_assert_eq!(self.strings, other.strings);

        debug_assert!(self.offsets.len() < other.offsets.len());
        // Append the missing chunk into Self
        self.offsets.extend_from(&other.offsets);
        debug_assert_eq!(self.offsets, other.offsets);
    }

    fn serialize_big_strings(&mut self, output: &mut SerializeStrings) {
        let start = self.to_serialize_index;

        for (start, end) in self.offsets.iter_from(start) {
            let start = *start as usize;
            let end = *end as usize;
            let string = self.strings.get(start..end).unwrap();

            let length: u32 = string.len() as u32;

            output.big_strings.extend_from_slice(&length.to_le_bytes());
            output.big_strings.extend_from_slice(string.as_bytes());
        }

        self.new_bytes_since_last_serialize = 0;
        self.to_serialize_index = self.offsets.len();
    }

    pub fn deallocate_serialized(&mut self) {
        let index = match self.to_serialize_index.checked_sub(1) {
            Some(index) => index,
            None => return,
        };

        let (start, _) = match self.offsets.get(index).copied() {
            Some(offsets) => offsets,
            None => return,
        };

        self.strings.deallocate_before(start as usize);
        self.offsets.deallocate_before(self.to_serialize_index);
    }

    fn new_bytes_since_last_serialize(&self) -> usize {
        self.new_bytes_since_last_serialize
    }

    fn deserialize(
        big_strings_file: File<{ TAG_BIG_STRINGS }>,
    ) -> Result<Self, DeserializationError> {
        // TODO: maybe start with higher capacity values knowing the file sizes

        let mut result = Self::default();

        let mut offset = big_strings_file.start();
        let end = big_strings_file.offset().as_u64();

        let mut length_bytes = [0u8; 4];
        let mut string_bytes = [0u8; 256];

        // big_strings_file is a sequence of
        // [u32 length le bytes | ... <length> bytes string]
        // big_strings_offsets_file is a sequence of:
        // [u32 start le bytes | u32 end le bytes]
        // but using `result.push_str` with the string seems to be enough to also update the offsets?

        let mut big_strings_file = big_strings_file.buffered()?;

        while offset < end {
            big_strings_file.read_exact(&mut length_bytes)?;

            let length = u32::from_le_bytes(length_bytes) as usize;
            offset += length_bytes.len() as u64;

            big_strings_file.read_exact(&mut string_bytes[0..length])?;
            offset += length as u64;

            let s = std::str::from_utf8(&string_bytes[..length])?;
            result.push_str(s);
        }

        result.to_serialize_index = result.offsets.len();

        Ok(result)
    }
}

#[derive(Clone)]
pub struct StringInterner {
    /// `Map` of hash of the string to their `StringId`
    /// We don't use `HashMap<String, StringId>` because the map would
    /// keep a copy of the string
    string_to_offset: SortedMap<u64, StringId>,
    /// Concatenation of all strings < STRING_INTERN_THRESHOLD.
    /// This is never cleared/deallocated
    all_strings: ChunkedString<{ 512 * 1024 }>,
    /// List of `StringId` that needs to be commited
    pub all_strings_to_serialize: ChunkedVec<StringId, 2048>,
    /// Concatenation of big strings. This is cleared/deallocated
    /// before every checkouts
    big_strings: BigStrings,

    new_bytes_since_last_serialize: usize,
}

impl std::fmt::Debug for StringInterner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StringInterner")
            .field(
                "string_to_offset_bytes",
                &self.string_to_offset.total_bytes(),
            )
            .field("all_strings_cap", &self.all_strings.capacity())
            .field(
                "all_strings_to_serialize_cap",
                &self.all_strings_to_serialize.capacity(),
            )
            .field("big_strings", &self.big_strings)
            .finish()
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self {
            string_to_offset: SortedMap::default(),
            all_strings: ChunkedString::new(), // ~512KB
            all_strings_to_serialize: ChunkedVec::default(),
            big_strings: BigStrings::default(),
            new_bytes_since_last_serialize: 0,
        } // Total ~69MB
    }
}

impl PartialEq for StringInterner {
    fn eq(&self, other: &Self) -> bool {
        self.all_strings.len() == other.all_strings.len() && self.big_strings == other.big_strings
    }
}

impl Eq for StringInterner {}

impl StringInterner {
    pub fn clone_after_reload(&mut self, other: &Self) {
        if self == other {
            return;
        }

        if !self.all_strings.is_empty() {
            eprintln!("StringInterner::clone_after_reload should be executed with empty self");
        }

        *self = other.clone();
    }

    /// This extends `Self::all_strings` and `Self::string_to_offset` from `other`.
    ///
    /// The other fields (`big_strings`) is not considered
    /// because this method is used to update the repository:
    /// The repository doesn't need that field.
    pub fn extend_from(&mut self, other: &Self) {
        if self == other {
            return;
        }

        if self.all_strings.len() != other.all_strings.len() {
            debug_assert!(self.all_strings.len() < other.all_strings.len());

            // Append the missing chunk into Self
            self.all_strings.extend_from(&other.all_strings);

            self.all_strings_to_serialize
                .extend_from_chunks(&other.all_strings_to_serialize);
        }

        debug_assert_eq!(self.all_strings, other.all_strings);

        self.big_strings.extend_from(&other.big_strings);
    }

    pub fn len(&self) -> usize {
        self.all_strings.len() + self.big_strings.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn make_string_id(&mut self, s: &str) -> StringId {
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

        let (index, length) = self.all_strings.push_str(s);

        assert_eq!(index & !0x3FFFFFF, 0);

        let index = index as u32;
        let length = length as u32;

        let string_id = StringId {
            bits: index << 5 | length,
        };

        self.string_to_offset.insert(hashed, string_id);
        self.all_strings_to_serialize.push(string_id);
        self.new_bytes_since_last_serialize += s.len();

        debug_assert_eq!(s, self.get_str(string_id).unwrap());

        string_id
    }

    pub fn get_str(&self, string_id: StringId) -> Result<Cow<str>, StorageError> {
        if string_id.is_big() {
            return self
                .big_strings
                .get_str(string_id.get_big_index())
                .ok_or(StorageError::StringNotFound);
        }

        let (start, end) = string_id.get_start_end();
        self.all_strings
            .get(start..end)
            .ok_or(StorageError::StringNotFound)
    }

    pub fn memory_usage(&self) -> StringsMemoryUsage {
        let all_strings_cap = self.all_strings.capacity();
        let big_strings_cap = self.big_strings.strings.capacity();
        let big_strings_hashes_bytes = self.big_strings.hashes.total_bytes();
        let all_strings_to_serialize_cap = self.all_strings_to_serialize.capacity();

        StringsMemoryUsage {
            all_strings_map_cap: self.string_to_offset.len(),
            all_strings_map_len: self.string_to_offset.len(),
            all_strings_to_serialize_cap,
            all_strings_cap,
            all_strings_len: self.all_strings.len(),
            big_strings_cap,
            big_strings_len: self.big_strings.strings.len(),
            big_strings_map_cap: self.big_strings.offsets.capacity(),
            big_strings_map_len: self.big_strings.offsets.len(),
            big_strings_hashes_bytes,
            total_bytes: all_strings_cap + big_strings_cap + big_strings_hashes_bytes,
        }
    }

    pub fn new_bytes_since_last_serialize(&self) -> usize {
        self.new_bytes_since_last_serialize + self.big_strings.new_bytes_since_last_serialize()
    }

    pub fn serialize(&mut self) -> SerializeStrings {
        let mut output = SerializeStrings {
            big_strings: Vec::with_capacity(1000),
            strings: Vec::with_capacity(1000),
        };

        for id in self.all_strings_to_serialize.iter() {
            let (start, end) = id.get_start_end();

            let string = self.all_strings.get(start..end).unwrap();
            let string = string.as_bytes();

            let length = string.len();
            let length: u8 = length.try_into().unwrap(); // never fail, the string is less than 30 bytes

            output.strings.push(length);
            output.strings.extend_from_slice(string);
        }

        self.new_bytes_since_last_serialize = 0;
        self.all_strings_to_serialize.clear();
        self.big_strings.serialize_big_strings(&mut output);

        output
    }

    pub fn deserialize(
        strings_file: File<{ TAG_STRINGS }>,
        big_strings_file: File<{ TAG_BIG_STRINGS }>,
    ) -> Result<Self, DeserializationError> {
        // TODO: maybe start with higher capacity values knowing the file sizes
        let mut result = Self::default();

        // Deserialize strings
        // Each entry is:
        // [length byte | ...<length> bytes string... ]
        // So it is read in sequence and then passed to `result.get_string_id` which will create the entry
        let mut offset = strings_file.start();
        let end = strings_file.offset().as_u64();

        let mut length_byte = [0u8; 1];
        let mut string_bytes = [0u8; 256]; //  30 should be enough here

        let mut strings_file = strings_file.buffered()?;

        while offset < end {
            strings_file.read_exact(&mut length_byte)?;
            offset += length_byte.len() as u64;

            let length = u8::from_le_bytes(length_byte) as usize;
            strings_file.read_exact(&mut string_bytes[..length])?;

            offset += length as u64;

            let s = std::str::from_utf8(&string_bytes[..length])?;
            let _string_id = result.make_string_id(s);

            // Need to keep this clear, everything being added has been serialized already
            result.all_strings_to_serialize.clear();
        }

        // Deserialize big strings

        result.big_strings = BigStrings::deserialize(big_strings_file)?;

        Ok(result)
    }

    pub fn set_to_serialize_index(&mut self, index: usize) {
        self.big_strings.to_serialize_index = index;
    }

    pub fn get_to_serialize_index(&self) -> usize {
        self.big_strings.to_serialize_index
    }

    pub fn shrink_to_fit(&mut self) {
        self.string_to_offset.shrink_to_fit();
        self.big_strings.hashes.shrink_to_fit();
    }

    pub fn deallocate_serialized(&mut self) {
        self.big_strings.deallocate_serialized();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_interner() {
        let mut interner = StringInterner::default();

        let a = interner.make_string_id("a");
        let b = interner.make_string_id("a");

        assert_eq!(a, b);
        assert!(!a.is_big());
        assert_eq!(interner.get_str(a).unwrap(), "a");
        assert_eq!(interner.get_str(a).unwrap(), interner.get_str(b).unwrap());

        let long_str = "a".repeat(STRING_INTERN_THRESHOLD);

        let a = interner.make_string_id(&long_str);
        let b = interner.make_string_id(&long_str);
        assert_eq!(a, b);
        assert!(a.is_big());
        assert_eq!(interner.get_str(a).unwrap(), long_str);
        assert_eq!(interner.get_str(b).unwrap(), long_str);

        // Make sure that StringInterner::extend_from works

        let mut other_interner = StringInterner::default();
        other_interner.extend_from(&interner);

        assert_eq!(interner.all_strings, other_interner.all_strings);
        assert_eq!(
            interner.big_strings.strings,
            other_interner.big_strings.strings
        );

        let long_str = "b".repeat(STRING_INTERN_THRESHOLD);
        let _ = interner.make_string_id(&long_str);

        // We added a big string to `interner`, it should be copied to `other_interner`.
        other_interner.extend_from(&interner);

        assert_eq!(interner.all_strings, other_interner.all_strings);
        assert_eq!(
            interner.big_strings.strings,
            other_interner.big_strings.strings
        );
    }
}
