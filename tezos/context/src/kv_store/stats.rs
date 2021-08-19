// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::mem;

use serde::Serialize;

use crate::hash::ObjectHash;
use crate::ContextValue;

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct StorageBackendStats {
    pub key_bytes: usize,
    pub value_bytes: usize,
    pub reused_keys_bytes: usize,
}

impl StorageBackendStats {
    /// increases `reused_keys_bytes` based on `key`
    pub fn update_reused_keys(&mut self, list: &HashSet<ObjectHash>) {
        self.reused_keys_bytes = list.capacity() * mem::size_of::<ObjectHash>();
    }

    pub fn total_as_bytes(&self) -> usize {
        self.key_bytes + self.value_bytes + self.reused_keys_bytes
    }
}

impl<'a> std::ops::Add<&'a Self> for StorageBackendStats {
    type Output = Self;

    fn add(self, other: &'a Self) -> Self::Output {
        Self {
            key_bytes: self.key_bytes + other.key_bytes,
            value_bytes: self.value_bytes + other.value_bytes,
            reused_keys_bytes: self.reused_keys_bytes + other.reused_keys_bytes,
        }
    }
}

impl std::ops::Add for StorageBackendStats {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        self + &other
    }
}

impl<'a> std::ops::AddAssign<&'a Self> for StorageBackendStats {
    fn add_assign(&mut self, other: &'a Self) {
        *self = *self + other;
    }
}

impl std::ops::AddAssign for StorageBackendStats {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl<'a> std::ops::Sub<&'a Self> for StorageBackendStats {
    type Output = Self;

    fn sub(self, other: &'a Self) -> Self::Output {
        Self {
            key_bytes: self.key_bytes - other.key_bytes,
            value_bytes: self.value_bytes - other.value_bytes,
            reused_keys_bytes: self.reused_keys_bytes - other.reused_keys_bytes,
        }
    }
}

impl std::ops::Sub for StorageBackendStats {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        self - &other
    }
}

impl<'a> std::ops::SubAssign<&'a Self> for StorageBackendStats {
    fn sub_assign(&mut self, other: &'a Self) {
        *self = *self - other;
    }
}

impl std::ops::SubAssign for StorageBackendStats {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl<'a> std::iter::Sum<&'a StorageBackendStats> for StorageBackendStats {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(StorageBackendStats::default(), |acc, cur| acc + cur)
    }
}

impl From<(&ObjectHash, &ContextValue)> for StorageBackendStats {
    fn from((_, value): (&ObjectHash, &ContextValue)) -> Self {
        StorageBackendStats {
            key_bytes: mem::size_of::<ObjectHash>(),
            value_bytes: size_of_vec(&value),
            reused_keys_bytes: 0,
        }
    }
}

pub fn size_of_vec<T>(v: &Vec<T>) -> usize {
    mem::size_of::<Vec<T>>() + mem::size_of::<T>() * v.capacity()
}
