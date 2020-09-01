// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
use crate::p2p::encoding::operations_for_blocks::{Path, PathLeft, PathRight};
use znfe::{FromOCaml, IntoRust, OCaml, OCamlBytes};

unsafe impl FromOCaml<Path> for Path {
    fn from_ocaml(v: OCaml<Path>) -> Self {
        if v.is_long() {
            Path::Op
        } else {
            match v.tag_value() {
                0 => {
                    let (path, right) = unsafe { v.field::<(Path, OCamlBytes)>(0).into_rust() };

                    Path::Left(Box::new(PathLeft::new(
                        path,
                        right,
                        Default::default(), // TODO: what is body?
                    )))
                }
                1 => {
                    let (left, path) = unsafe { v.field::<(OCamlBytes, Path)>(0).into_rust() };
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
