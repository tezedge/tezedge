// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod bin;
mod enc;
mod encoding;
mod gen;
mod make;
mod nom;
mod symbol;

#[proc_macro_derive(HasEncoding, attributes(encoding))]
pub fn derive_tezos_encoding(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let encoding = match crate::make::make_encoding(&input) {
        Ok(encoding) => encoding,
        Err(e) => return e.into_compile_error().into(),
    };
    let tokens = crate::enc::generate_encoding_for_data(&encoding);
    tokens.into()
}

#[proc_macro_derive(NomReader, attributes(encoding))]
pub fn derive_nom_reader(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let encoding = match crate::make::make_encoding(&input) {
        Ok(encoding) => encoding,
        Err(e) => return e.into_compile_error().into(),
    };
    let tokens = crate::nom::generate_nom_read_for_data(&encoding);
    tokens.into()
}

#[proc_macro_derive(BinWriter, attributes(encoding))]
pub fn derive_bin_writer(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let encoding = match crate::make::make_encoding(&input) {
        Ok(encoding) => encoding,
        Err(e) => return e.into_compile_error().into(),
    };
    let tokens = crate::bin::generate_bin_write_for_data(&encoding);
    tokens.into()
}

#[proc_macro_derive(Generated, attributes(encoding))]
pub fn derive_generated_writer(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let encoding = match crate::make::make_encoding(&input) {
        Ok(encoding) => encoding,
        Err(e) => return e.into_compile_error().into(),
    };
    let tokens = crate::gen::generate_for_data(&encoding);
    tokens.into()
}
