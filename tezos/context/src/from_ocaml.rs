// Copyright {c} SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use ocaml_interop::{impl_from_ocaml_polymorphic_variant, impl_from_ocaml_variant, OCamlInt};
use tezos_timing::QueryKind;

use crate::working_tree::working_tree::{FoldDepth, FoldOrder};

pub struct OCamlQueryKind;

impl_from_ocaml_polymorphic_variant! {
    FoldDepth {
        Eq(n: OCamlInt) => FoldDepth::Eq(n),
        Le(n: OCamlInt) => FoldDepth::Le(n),
        Lt(n: OCamlInt) => FoldDepth::Lt(n),
        Ge(n: OCamlInt) => FoldDepth::Ge(n),
        Gt(n: OCamlInt) => FoldDepth::Gt(n),
    }
}

impl_from_ocaml_polymorphic_variant! {
    FoldOrder {
        Sorted => FoldOrder::Sorted,
        Undefined => FoldOrder::Undefined,
    }
}

impl_from_ocaml_variant! {
    OCamlQueryKind => QueryKind {
        QueryKind::Mem,
        QueryKind::MemTree,
        QueryKind::Find,
        QueryKind::FindTree,
        QueryKind::Add,
        QueryKind::AddTree,
        QueryKind::Remove,
    }
}
