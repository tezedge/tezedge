// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
use crate::p2p::encoding::operations_for_blocks::{Path, PathLeft, PathRight};
use crypto::hash::Hash;
use znfe::{FromOCaml, IntoRust, OCaml, OCamlBytes};

struct OCamlHash {}

unsafe impl FromOCaml<OCamlHash> for Hash {
    fn from_ocaml(v: OCaml<OCamlHash>) -> Self {
        unsafe { v.field::<OCamlBytes>(0).into_rust() }
    }
}

unsafe impl FromOCaml<Path> for Path {
    fn from_ocaml(v: OCaml<Path>) -> Self {
        if v.is_long() {
            Path::Op
        } else {
            match v.tag_value() {
                0 => {
                    let path = unsafe { v.field::<Path>(0).into_rust() };
                    let right = unsafe { v.field::<OCamlHash>(1).into_rust() };

                    Path::Left(Box::new(PathLeft::new(
                        path,
                        right,
                        Default::default(), // TODO: what is body?
                    )))
                }
                1 => {
                    let left = unsafe { v.field::<OCamlHash>(0).into_rust() };
                    let path = unsafe { v.field::<Path>(1).into_rust() };

                    Path::Right(Box::new(PathRight::new(
                        left,
                        path,
                        Default::default(), // TODO: what is body?
                    )))
                }
                tag => panic!("Invalid tag value for OCaml<Path>: {}", tag),
            }
        }
    }
}
