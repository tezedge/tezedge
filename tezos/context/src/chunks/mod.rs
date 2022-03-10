// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod shared_vec;
pub mod string;
pub mod vec;

pub use shared_vec::*;
pub use string::*;
pub use vec::*;

type Chunk<T> = Vec<T>;

const DEFAULT_LIST_LENGTH: usize = 16;
