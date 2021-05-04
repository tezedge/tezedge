extern crate proc_macro;

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{parse_macro_input, DeriveInput, Meta, Type};

mod symbol;

/// Checks that the type is `Vec<u8>`
fn is_vec_u8(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => {
            if path.path.segments.len() == 1 {
                let segm = path.path.segments.last().unwrap();
                if segm.ident == symbol::rust::VEC {
                    match &segm.arguments {
                        syn::PathArguments::AngleBracketed(args) => {
                            if args.args.len() == 1 {
                                match args.args.last().unwrap() {
                                    syn::GenericArgument::Type(Type::Path(path))
                                        if path.path == symbol::rust::U8 =>
                                    {
                                        return true
                                    }
                                    _ => (),
                                }
                            }
                        }
                        _ => (),
                    }
                }
            }
        }
        _ => (),
    };
    false
}

fn is_string(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) if path.path == symbol::rust::STRING => true,
        _ => false,
    }
}

/// Finds `encoding` attribute and parse its content
fn get_encoding_meta(attrs: &[syn::Attribute]) -> Result<Option<syn::Meta>, syn::Error> {
    let encoding_attr = attrs.iter().find(|attr| attr.path == symbol::ENCODING);
    match encoding_attr {
        None => Ok(None),
        Some(encoding_meta) => {
            let meta = encoding_meta.parse_meta();
            match meta {
                Ok(syn::Meta::List(m)) => {
                    if m.nested.len() != 1 {
                        return Err(syn::Error::new_spanned(
                            &encoding_meta.to_token_stream(),
                            "Unexpected number of `encoding` attrubute nested attributes",
                        ));
                    }
                    match m.nested.into_iter().next().unwrap() {
                        syn::NestedMeta::Meta(m) => Ok(Some(m)),
                        _ => Err(syn::Error::new_spanned(
                            &encoding_meta.to_token_stream(),
                            "Unexpected kind of `encoding` nested attribute",
                        )),
                    }
                }
                Ok(_) => Err(syn::Error::new_spanned(
                    &encoding_meta.to_token_stream(),
                    format!(
                        "Unexpected kind of `encoding` attrubute: {:?}",
                        encoding_meta
                    ),
                )),
                Err(e) => Err(e),
            }
        }
    }
}

/// Handles different kinds of encoding specified in attrubutes
trait EncodingHandler {
    /// Handles bare type, as if no encoding has been specified.
    fn on_type(&self, ty: &syn::Type) -> TokenStream;
    /// Handles `Bytes` encoding.
    fn on_bytes(&self) -> TokenStream;
    /// Handles `String` encoding.
    fn on_string(&self) -> TokenStream;
    /// Handles `BoundedString(size)` encoding.
    fn on_bounded_string(&self, size: &syn::Expr) -> TokenStream;
    /// Handles `Sized(size, inner)` encoding.
    fn on_sized(&self, ty: &syn::Type, size: &syn::Expr, inner: Option<&syn::Meta>) -> TokenStream;
}

fn spanned_error<T: ToTokens, U: std::fmt::Display>(tokens: T, message: U) -> TokenStream {
    syn::Error::new_spanned(tokens, message).into_compile_error()
}

fn attribute_error<T: ToTokens, U: std::fmt::Display>(
    name: &syn::Path,
    tokens: T,
    message: U,
) -> TokenStream {
    spanned_error(
        tokens,
        format!("Attribute `{}`: {}", quote::quote! {#name}, message),
    )
}

/// Visits encoding attrubutes and calls `handler`'s appropriate method, see [EncodingHandler].
struct EncodingVisitor {}

impl EncodingVisitor {
    fn _modifier<'a>(meta_list: &'a syn::MetaList) -> Result<Option<&'a syn::Meta>, TokenStream> {
        let mut it = meta_list.nested.iter();
        let inner = match it.next() {
            Some(syn::NestedMeta::Meta(m)) => Some(m),
            None => None,
            _ => {
                return Err(attribute_error(
                    &meta_list.path,
                    &meta_list,
                    "Wrong `inner` parameter",
                ))
            }
        };
        Ok(inner)
    }

    fn parameterized_modifier<'a>(
        meta_list: &'a syn::MetaList,
        param: &str,
    ) -> Result<(syn::Expr, Option<&'a syn::Meta>), TokenStream> {
        let mut it = meta_list.nested.iter();
        let param = match it.next() {
            Some(syn::NestedMeta::Lit(syn::Lit::Str(s))) => match s.parse() {
                Ok(param) => param,
                Err(e) => return Err(e.into_compile_error()),
            },
            Some(_) => {
                return Err(attribute_error(
                    &meta_list.path,
                    &meta_list,
                    format!("Expecting string literal as `{}` attribute", param),
                ))
            }
            None => {
                return Err(attribute_error(
                    &meta_list.path,
                    &meta_list,
                    format!("Missing `{}` attribute", param),
                ))
            }
        };
        let inner = match it.next() {
            Some(syn::NestedMeta::Meta(m)) => Some(m),
            None => None,
            _ => {
                return Err(attribute_error(
                    &meta_list.path,
                    &meta_list,
                    "Wrong `inner` parameter",
                ))
            }
        };
        Ok((param, inner))
    }

    fn parameterized_encoding(
        meta_list: &syn::MetaList,
        param: &str,
    ) -> Result<syn::Expr, TokenStream> {
        let mut it = meta_list.nested.iter();
        let param = match it.next() {
            Some(syn::NestedMeta::Lit(syn::Lit::Str(s))) => match s.parse() {
                Ok(param) => param,
                Err(e) => return Err(e.into_compile_error()),
            },
            Some(_) => {
                return Err(attribute_error(
                    &meta_list.path,
                    &meta_list,
                    format!("Expecting string literal as `{}` attribute", param),
                ))
            }
            None => {
                return Err(attribute_error(
                    &meta_list.path,
                    &meta_list,
                    format!("Missing `{}` attribute", param),
                ))
            }
        };
        Ok(param)
    }

    pub fn visit<T: EncodingHandler>(
        &self,
        ty: &Type,
        meta: Option<&Meta>,
        handler: &T,
    ) -> TokenStream {
        match meta {
            // `Sized("Expr")`, `Sized("Expr", Inner)`
            Some(syn::Meta::List(m)) if m.path == symbol::SIZED => {
                match Self::parameterized_modifier(m, "size") {
                    Ok((size, inner)) => handler.on_sized(ty, &size, inner),
                    Err(ts) => ts,
                }
            }
            // `Bytes`
            Some(syn::Meta::Path(p)) if p == symbol::BYTES => {
                if is_vec_u8(ty) {
                    handler.on_bytes()
                } else {
                    spanned_error(&ty, "`Bytes` encoding is supported only for `Vec<u8>` type")
                }
            }
            // `String`
            Some(syn::Meta::Path(p)) if p == symbol::STRING => {
                if is_string(ty) {
                    handler.on_string()
                } else {
                    spanned_error(&ty, "`String` encoding is supported only for `String` type")
                }
            }
            // `BoundedString("SIZE")`
            Some(Meta::List(m)) if m.path == symbol::BOUNDED_STRING => {
                match Self::parameterized_encoding(m, "size") {
                    Ok(size) => {
                        if is_string(ty) {
                            handler.on_bounded_string(&size)
                        } else {
                            spanned_error(
                                &ty,
                                "`BoundedString(...)` encoding is supported only for `String` type",
                            )
                        }
                    }
                    Err(ts) => ts,
                }
            }
            // no attributes, use type information
            None => handler.on_type(ty),
            Some(Meta::List(m)) => attribute_error(&m.path, &m, "Encoding not implemented"),
            Some(Meta::Path(m)) => attribute_error(&m, &m, "Encoding not implemented"),
            _ => spanned_error(meta, "Invalid meta attribute"),
        }
    }
}

mod enc {
    use lazy_static::lazy_static;
    use proc_macro2::TokenStream;
    use quote::quote;
    use syn::spanned::Spanned;

    use crate::{
        get_encoding_meta, spanned_error, symbol::Symbol, EncodingHandler, EncodingVisitor,
    };

    lazy_static! {
        static ref DIRECT_MAPPING: std::collections::HashMap<Symbol, &'static str> = {
            use crate::symbol::rust::*;
            use crate::symbol::*;
            [(U8, UINT_8), (U16, UINT_16)].iter().cloned().collect()
        };
    }

    /// Returns direct mapping option for the specified type encoding,
    /// e.g. `u8` -> `Encoding::Uint8`.
    fn get_direct_mapping(path: &syn::Path) -> Option<TokenStream> {
        let len = path.segments.len();
        if len > 2 {
            return None;
        }
        let mut it = path.segments.iter();
        if len == 2 && it.next().unwrap().ident != "std" {
            return None;
        }
        DIRECT_MAPPING
            .get(it.next().unwrap().ident.to_string().as_str())
            .map(|s| {
                let ident = syn::Ident::new(&format!("{}", s), path.span());
                quote! {
                    tezos_encoding::encoding::Encoding::#ident
                }
            })
    }

    struct EncodingGenerator {
        visitor: EncodingVisitor,
    }

    impl EncodingGenerator {
        fn new() -> Self {
            let visitor = EncodingVisitor {};
            EncodingGenerator { visitor }
        }
        fn generate(&self, ty: &syn::Type, meta: Option<&syn::Meta>) -> TokenStream {
            self.visitor.visit(ty, meta, self)
        }
    }

    impl EncodingHandler for EncodingGenerator {
        fn on_type(&self, ty: &syn::Type) -> TokenStream {
            match ty {
                syn::Type::Path(path) => {
                    if let Some(tokens) = get_direct_mapping(&path.path) {
                        tokens.into()
                    } else {
                        quote! { #ty::encoding().clone() }
                    }
                }
                _ => spanned_error(&ty, "Encoding not implemented for this type"),
            }
        }

        fn on_bytes(&self) -> TokenStream {
            quote! { tezos_encoding::encoding::Encoding::Bytes }
        }

        fn on_string(&self) -> TokenStream {
            quote! {
                tezos_encoding::encoding::Encoding::String
            }
        }

        fn on_bounded_string(&self, size: &syn::Expr) -> TokenStream {
            quote! {
                tezos_encoding::encoding::Encoding::BoundedString(#size)
            }
        }

        fn on_sized(
            &self,
            ty: &syn::Type,
            size: &syn::Expr,
            inner: Option<&syn::Meta>,
        ) -> TokenStream {
            let inner = self.visitor.visit(ty, inner, self);
            quote! {
                tezos_encoding::encoding::Encoding::Sized(#size, Box::new(#inner))
            }
        }
    }

    fn generate_field_encoding(field: &syn::Field) -> TokenStream {
        get_encoding_meta(&field.attrs)
            .map(|meta| EncodingGenerator::new().generate(&field.ty, meta.as_ref()))
            .unwrap_or_else(|err| err.to_compile_error())
    }

    pub fn struct_fields_encoding(data: &syn::Data) -> TokenStream {
        match data {
            syn::Data::Struct(data) => match data.fields {
                syn::Fields::Named(ref fields) => {
                    let recurse = fields.named.iter().map(|f| {
                        let name = f
                            .ident
                            .as_ref()
                            .map(|i| format!("{}", &i))
                            .unwrap_or(String::new());
                        let encoding = generate_field_encoding(f);
                        quote! {
                            tezos_encoding::encoding::Field::new(#name, #encoding),
                        }
                    });
                    quote! {
                        #(#recurse)*
                    }
                }
                _ => spanned_error(&data.fields, "Only `struct` with named fields supported"),
            },
            syn::Data::Enum(e) => spanned_error(e.enum_token, "Only `struct` types supported"),
            syn::Data::Union(e) => spanned_error(e.union_token, "Only `struct` types supported"),
        }
    }

    pub fn derive_encoding(input: syn::DeriveInput) -> TokenStream {
        let name = &input.ident;
        let name_str = name.to_string();
        let encoding_static_name = syn::Ident::new(
            &format!("__TEZOS_ENCODING_{}", name.to_string().to_uppercase()),
            name.span(),
        );
        let fields_encoding = struct_fields_encoding(&input.data);
        quote! {
            lazy_static::lazy_static! {
                #[allow(non_upper_case_globals)]
                static ref #encoding_static_name: tezos_encoding::encoding::Encoding = tezos_encoding::encoding::Encoding::Obj(
                    #name_str,
                    vec![
                        #fields_encoding
                    ]
                );
            }

            impl tezos_encoding::encoding::HasEncoding for #name {
                fn encoding() -> &'static tezos_encoding::encoding::Encoding {
                    &#encoding_static_name
                }
            }
        }
    }
}

mod nom {
    use proc_macro2::TokenStream;
    use quote::{quote, ToTokens};

    use crate::{get_encoding_meta, spanned_error, symbol, EncodingHandler, EncodingVisitor};

    struct NomReaderGenerator {
        visitor: EncodingVisitor,
    }

    impl NomReaderGenerator {
        fn new() -> Self {
            let visitor = EncodingVisitor {};
            NomReaderGenerator { visitor }
        }
        fn generate(&self, ty: &syn::Type, meta: Option<&syn::Meta>) -> TokenStream {
            self.visitor.visit(ty, meta, self)
        }
    }

    impl EncodingHandler for NomReaderGenerator {
        fn on_type(&self, ty: &syn::Type) -> TokenStream {
            match ty {
                syn::Type::Path(path) => {
                    if path.path == symbol::rust::U16 {
                        quote! {
                            nom::number::complete::u16(nom::number::Endianness::Big)
                        }
                    } else {
                        quote! {
                            <#ty as tezos_encoding::nom::NomReader>::from_bytes
                        }
                    }
                }
                _ => spanned_error(&ty, "Encoding not implemented for this type"),
            }
        }

        fn on_bytes(&self) -> TokenStream {
            quote! {
                nom::combinator::map(nom::combinator::rest, Vec::from)
            }
        }

        fn on_string(&self) -> TokenStream {
            quote! {
                nom::combinator::map_res(
                    nom::combinator::flat_map(
                        nom::number::complete::u32(nom::number::Endianness::Big),
                        nom::bytes::complete::take,
                    ),
                    |bytes| std::str::from_utf8(bytes).map(str::to_string),
                )
            }
        }

        fn on_bounded_string(&self, size: &syn::Expr) -> TokenStream {
            quote! {
                nom::combinator::map_res(
                    nom::combinator::flat_map(
                        nom::combinator::verify(
                            nom::number::complete::u32(nom::number::Endianness::Big),
                            |v| *v <= #size as u32,
                        ),
                        nom::bytes::complete::take,
                    ),
                    |bytes| std::str::from_utf8(bytes).map(str::to_string),
                )
            }
        }

        fn on_sized(
            &self,
            ty: &syn::Type,
            size: &syn::Expr,
            inner: Option<&syn::Meta>,
        ) -> TokenStream {
            let inner = self.visitor.visit(ty, inner, self);
            quote! {
                nom::combinator::map_parser(
                    nom::bytes::complete::take(#size),
                    #inner
                )
            }
        }
    }

    fn generate_field_nom_reader(field: &syn::Field) -> TokenStream {
        get_encoding_meta(&field.attrs)
            .map(|meta| NomReaderGenerator::new().generate(&field.ty, meta.as_ref()))
            .unwrap_or_else(|err| err.into_compile_error())
    }

    fn struct_fields_nom_reader(data: &syn::Data, name: &syn::Ident) -> TokenStream {
        match data {
            syn::Data::Struct(data) => match data.fields {
                syn::Fields::Named(ref fields) => {
                    let nom_reader = fields.named.iter().map(|f| {
                        let name = &f.ident;
                        let nom_read = generate_field_nom_reader(f);
                        quote! {
                            let (bytes, #name) = #nom_read(bytes)?;
                        }
                    });
                    let construct = fields
                        .named
                        .iter()
                        .map(|f| f.ident.as_ref().map(|i| i.to_token_stream()));
                    quote! {
                        #(#nom_reader)*
                        Ok((bytes, #name { #(#construct, )* }))
                    }
                }
                _ => spanned_error(&data.fields, "Only `struct` with named fields supported"),
            },
            syn::Data::Enum(e) => spanned_error(e.enum_token, "Only `struct` types supported"),
            syn::Data::Union(e) => spanned_error(e.union_token, "Only `struct` types supported"),
        }
    }

    pub fn derive_nom_reader(input: syn::DeriveInput) -> TokenStream {
        let name = &input.ident;
        let fields_nom_read = struct_fields_nom_reader(&input.data, name);
        quote! {
            impl tezos_encoding::nom::NomReader for #name {
                fn from_bytes(bytes: &[u8]) -> nom::IResult<&[u8], Self> {
                    #fields_nom_read
                }
            }
        }
    }
}

#[proc_macro_derive(HasEncoding, attributes(encoding))]
pub fn derive_tezos_encoding(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let expanded = enc::derive_encoding(input);
    expanded.into()
}

#[proc_macro_derive(NomReader, attributes(encoding))]
pub fn derive_nom_reader(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let expanded = nom::derive_nom_reader(input);
    expanded.into()
}

/*
fn get_lit_str(lit: Lit) -> String {
    match lit {
        Lit::Str(s) => s.value(),
        _ => unimplemented!(),
    }
}

fn get_encoding_meta_items(attr: &Attribute) -> Vec<NestedMeta> {
    if attr.path != symbol::ENCODING {
        return vec![];
    }

    match attr.parse_meta().expect("error parsing meta attributes") {
        Meta::List(meta) => meta.nested.into_iter().collect(),
        _ => unimplemented!(),
    }
}
*/
