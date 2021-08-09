#![feature(allocator_api)]

use crate::errors::EdgeKVError;

pub mod datastore;
pub mod errors;
pub mod file_ops;
pub mod edgekv;
pub mod schema;
pub type Result<T> = std::result::Result<T, EdgeKVError>;

#[cfg(test)]
mod tests;
