// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod slice;
pub mod string;
pub mod vec;

pub use slice::*;
pub use string::*;
pub use vec::*;

type Chunk<T> = Vec<T>;

const DEFAULT_LIST_LENGTH: usize = 10;
