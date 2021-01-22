// Copyright {c} SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![allow(unused_assignments)]
#![allow(unused_mut)]
#![allow(unused_variables)]

use ocaml_interop::{OCamlBytes, OCamlFloat, OCamlInt64, OCamlList, ToRust};

use crate::channel::ContextAction;

// TODO: temporary, remove all these macros once ocaml-interop has been upgraded
macro_rules! custom_impl_from_ocaml_variant {
    ($ocaml_typ:ty => $rust_typ:ty {
        $($t:tt)*
    }) => {
        unsafe impl ocaml_interop::FromOCaml<$ocaml_typ> for $rust_typ {
            fn from_ocaml(v: ocaml_interop::OCaml<$ocaml_typ>) -> Self {
                let result = custom_ocaml_unpack_variant! {
                    v => {
                        $($t)*
                    }
                };

                let msg = concat!(
                    "Failure when unpacking an OCaml<", stringify!($ocaml_typ), "> variant into ",
                    stringify!($rust_typ), " (unexpected tag value)");

                result.expect(msg)
            }
        }
    };

    ($both_typ:ty {
        $($t:tt)*
    }) => {
        custom_impl_from_ocaml_variant!{
            $both_typ => $both_typ {
                $($t)*
            }
        }
    };
}

#[macro_export]
macro_rules! custom_ocaml_unpack_variant {
    ($self:ident => {
        $($($tag:ident)::+ $({$($slot_name:ident: $slot_typ:ty),+ $(,)?})? $(=> $conv:expr)?),+ $(,)?
    }) => {
        (|| {
            let mut current_block_tag = 0;
            let mut current_long_tag = 0;

            $(
                custom_unpack_variant_tag!(
                    $self, current_block_tag, current_long_tag,
                    $($tag)::+ $({$($slot_name: $slot_typ),+})? $(=> $conv)?);
            )+

            Err("Invalid tag value found when converting from an OCaml variant")
        })()
    };
}

#[macro_export]
macro_rules! custom_unpack_variant_tag {
    ($self:ident, $current_block_tag:ident, $current_long_tag:ident, $($tag:ident)::+) => {
        custom_unpack_variant_tag!($self, $current_block_tag, $current_long_tag, $($tag)::+ => $($tag)::+)
    };

    ($self:ident, $current_block_tag:ident, $current_long_tag:ident, $($tag:ident)::+ => $conv:expr) => {
        if $self.is_long() && ocaml_interop::internal::int_val(unsafe { $self.raw() }) == $current_long_tag {
            return Ok($conv);
        }
        $current_long_tag += 1;
    };

    // Braces: record
    ($self:ident, $current_block_tag:ident, $current_long_tag:ident,
        $($tag:ident)::+ {$($slot_name:ident: $slot_typ:ty),+}) => {

            custom_unpack_variant_tag!(
            $self, $current_block_tag, $current_long_tag,
            $($tag)::+ {$($slot_name: $slot_typ),+} => $($tag)::+{$($slot_name),+})
    };

    // Braces: record
    ($self:ident, $current_block_tag:ident, $current_long_tag:ident,
        $($tag:ident)::+ {$($slot_name:ident: $slot_typ:ty),+} => $conv:expr) => {

        if $self.is_block() && $self.tag_value() == $current_block_tag {
            let mut current_field = 0;

            $(
                let $slot_name = unsafe { $self.field::<$slot_typ>(current_field).to_rust() };
                current_field += 1;
            )+

            return Ok($conv);
        }
        $current_block_tag += 1;
    };
}

custom_impl_from_ocaml_variant! {
    ContextAction {
        ContextAction::Set {
            context_hash: Option<OCamlBytes>,
            block_hash: Option<OCamlBytes>,
            operation_hash: Option<OCamlBytes>,
            tree_hash: OCamlBytes,
            new_tree_hash: OCamlBytes,
            start_time: OCamlFloat,
            end_time: OCamlFloat,
            key: OCamlList<OCamlBytes>,
            value: OCamlBytes,
            value_as_json: Option<OCamlBytes>,
        },
        ContextAction::Delete {
            context_hash: Option<OCamlBytes>,
            block_hash: Option<OCamlBytes>,
            operation_hash: Option<OCamlBytes>,
            tree_hash: OCamlBytes,
            new_tree_hash: OCamlBytes,
            start_time: OCamlFloat,
            end_time: OCamlFloat,
            key: OCamlList<OCamlBytes>,
        },
        ContextAction::RemoveRecursively {
            context_hash: Option<OCamlBytes>,
            block_hash: Option<OCamlBytes>,
            operation_hash: Option<OCamlBytes>,
            tree_hash: OCamlBytes,
            new_tree_hash: OCamlBytes,
            start_time: OCamlFloat,
            end_time: OCamlFloat,
            key: OCamlList<OCamlBytes>,
        },
        ContextAction::Copy {
            context_hash: Option<OCamlBytes>,
            block_hash: Option<OCamlBytes>,
            operation_hash: Option<OCamlBytes>,
            tree_hash: OCamlBytes,
            new_tree_hash: OCamlBytes,
            start_time: OCamlFloat,
            end_time: OCamlFloat,
            from_key: OCamlList<OCamlBytes>,
            to_key: OCamlList<OCamlBytes>,
        },
        ContextAction::Checkout {
            context_hash: OCamlBytes,
            start_time: OCamlFloat,
            end_time: OCamlFloat,
        },
        ContextAction::Commit {
            parent_context_hash: Option<OCamlBytes>,
            block_hash: Option<OCamlBytes>,
            new_context_hash: OCamlBytes,
            tree_hash: OCamlBytes,
            start_time: OCamlFloat,
            end_time: OCamlFloat,
            author: OCamlBytes,
            message: OCamlBytes,
            date: OCamlInt64,
            parents: OCamlList<OCamlBytes>,
        },
        ContextAction::Mem {
            context_hash: Option<OCamlBytes>,
            block_hash: Option<OCamlBytes>,
            operation_hash: Option<OCamlBytes>,
            tree_hash: OCamlBytes,
            start_time: OCamlFloat,
            end_time: OCamlFloat,
            key: OCamlList<OCamlBytes>,
            value: bool,
        },
        ContextAction::DirMem {
            context_hash: Option<OCamlBytes>,
            block_hash: Option<OCamlBytes>,
            operation_hash: Option<OCamlBytes>,
            tree_hash: OCamlBytes,
            start_time: OCamlFloat,
            end_time: OCamlFloat,
            key: OCamlList<OCamlBytes>,
            value: bool,
        },
        ContextAction::Get {
            context_hash: Option<OCamlBytes>,
            block_hash: Option<OCamlBytes>,
            operation_hash: Option<OCamlBytes>,
            tree_hash: OCamlBytes,
            start_time: OCamlFloat,
            end_time: OCamlFloat,
            key: OCamlList<OCamlBytes>,
            value: OCamlBytes,
            value_as_json: Option<OCamlBytes>,
        },
        ContextAction::Fold {
            context_hash: Option<OCamlBytes>,
            block_hash: Option<OCamlBytes>,
            operation_hash: Option<OCamlBytes>,
            tree_hash: OCamlBytes,
            start_time: OCamlFloat,
            end_time: OCamlFloat,
            key: OCamlList<OCamlBytes>,
        },
    }
}
