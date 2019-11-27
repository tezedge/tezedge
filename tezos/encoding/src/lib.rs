#![feature(concat_idents, const_fn, const_if_match)]
mod types;
mod bit_utils;

#[macro_use]
pub mod hash;
pub mod encoding;

pub mod de;
pub mod ser;

pub mod binary_reader;
pub mod binary_writer;
pub mod json_writer;