// Copyright {c} SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use ocaml_interop::{impl_from_ocaml_polymorphic_variant, OCamlInt};

use crate::working_tree::working_tree::FoldDepth;

impl_from_ocaml_polymorphic_variant! {
    FoldDepth {
        Eq(n: OCamlInt) => FoldDepth::Eq(n),
        Le(n: OCamlInt) => FoldDepth::Le(n),
        Lt(n: OCamlInt) => FoldDepth::Lt(n),
        Ge(n: OCamlInt) => FoldDepth::Ge(n),
        Gt(n: OCamlInt) => FoldDepth::Gt(n),
    }
}
