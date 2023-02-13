// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    borrow::Borrow,
    fmt::{self, Display},
};
use syn::{Ident, Path};

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
pub struct Symbol(&'static str);

pub mod rust {
    use super::*;

    pub const I8: Symbol = Symbol("i8");
    pub const U8: Symbol = Symbol("u8");
    pub const I16: Symbol = Symbol("i16");
    pub const U16: Symbol = Symbol("u16");
    pub const I32: Symbol = Symbol("i32");
    pub const U32: Symbol = Symbol("u32");
    pub const I64: Symbol = Symbol("i64");
    pub const F64: Symbol = Symbol("f64");
    pub const BOOL: Symbol = Symbol("bool");

    pub const STRING: Symbol = Symbol("String");

    pub const VEC: Symbol = Symbol("Vec");
    pub const OPTION: Symbol = Symbol("Option");

    pub const _BIG_INT: Symbol = Symbol("BigInt");
}

/// Built-in primitive encoding to use.
pub const BUILTIN: Symbol = Symbol("builtin");

/// Kind of built-in primitive encoding to use.
pub const KIND: Symbol = Symbol("kind");

/// Composite encoding
pub const COMPOSITE: Symbol = Symbol("composite");

/// Attribute name used to mark field/variant as ignored.
pub const SKIP: Symbol = Symbol("skip");
pub const HASH: Symbol = Symbol("hash");

/// Attribute used to specify maximal size/lengh.
pub const MAX: Symbol = Symbol("max");

pub const ENCODING: Symbol = Symbol("encoding");
pub const BYTES: Symbol = Symbol("bytes");
pub const STRING: Symbol = Symbol("string");
pub const OPTION: Symbol = Symbol("option");

pub const TIMESTAMP: Symbol = Symbol("timestamp");

pub const SIZED: Symbol = Symbol("sized");
pub const SIZE: Symbol = Symbol("size");

pub const LIST: Symbol = Symbol("list");
pub const BOUNDED: Symbol = Symbol("bounded");
pub const DYNAMIC: Symbol = Symbol("dynamic");
pub const SHORT_DYNAMIC: Symbol = Symbol("short_dynamic");

pub const TAGS: Symbol = Symbol("tags");
pub const IGNORE_UNKNOWN: Symbol = Symbol("ignore_unknown");
pub const TAG: Symbol = Symbol("tag");

pub const Z_ARITH: Symbol = Symbol("zarith");
pub const MU_TEZ: Symbol = Symbol("mutez");

pub const RESERVE: Symbol = Symbol("reserve");

impl PartialEq<Symbol> for Ident {
    fn eq(&self, word: &Symbol) -> bool {
        self == word.0
    }
}

impl<'a> PartialEq<Symbol> for &'a Ident {
    fn eq(&self, word: &Symbol) -> bool {
        *self == word.0
    }
}

impl PartialEq<Symbol> for Path {
    fn eq(&self, word: &Symbol) -> bool {
        self.is_ident(word.0)
    }
}

impl<'a> PartialEq<Symbol> for &'a Path {
    fn eq(&self, word: &Symbol) -> bool {
        self.is_ident(word.0)
    }
}

impl Display for Symbol {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Borrow<str> for Symbol {
    fn borrow(&self) -> &str {
        self.0
    }
}
