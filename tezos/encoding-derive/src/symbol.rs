use std::{borrow::Borrow, fmt::{self, Display}};
use syn::{Ident, Path};

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
pub struct Symbol(&'static str);

pub mod rust {
    use super::*;

    pub const STRING: Symbol = Symbol("String");
    pub const U8: Symbol = Symbol("u8");
    pub const U16: Symbol = Symbol("u16");
    pub const VEC: Symbol = Symbol("Vec");
}

pub const UINT_8: &str ="Uint8";
pub const UINT_16: &str ="Uint16";

pub const ENCODING: Symbol = Symbol("encoding");
pub const SIZED: Symbol = Symbol("Sized");
pub const BYTES: Symbol = Symbol("Bytes");
pub const STRING: Symbol = Symbol("String");
pub const BOUNDED_STRING: Symbol = Symbol("BoundedString");

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
