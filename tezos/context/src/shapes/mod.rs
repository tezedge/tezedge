mod ondisk;

use crate::working_tree::string_interner::StringId;
use std::borrow::Cow;

pub use ondisk::*;

pub enum ShapeStrings<'a> {
    SliceIds(Cow<'a, [StringId]>),
    Owned(Vec<String>),
}

impl<'a> ShapeStrings<'a> {
    pub fn len(&self) -> usize {
        match self {
            ShapeStrings::SliceIds(slice) => slice.len(),
            ShapeStrings::Owned(owned) => owned.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
