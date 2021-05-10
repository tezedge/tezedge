extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod common;
mod encoding;
mod symbol;
mod make;
mod enc;
mod nom;

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

/*

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
///
///
/// /// Visit Rust data declaration suitable for encoding
struct DataVisitor {}

trait DataHandler {
    fn on_struct(&self, data: &syn::DataStruct, err: &Errors) -> TokenStream;
    fn on_field(&self, field: &syn::Field, err: &Errors) -> TokenStream;
    fn on_enum(&self, data: &syn::DataEnum, err: &Errors) -> TokenStream;
    fn on_variant(&self, variant: &syn::Variant, err: &Errors) -> TokenStream;
}

impl DataVisitor {
    pub fn visit_data(
        &self,
        data: &syn::Data,
        err: &Errors,
        handler: impl DataHandler,
    ) -> TokenStream {
        match data {
            syn::Data::Struct(data) => handler.on_struct(data, err),
            syn::Data::Enum(data) => handler.on_enum(data, err),
            syn::Data::Union(e) => {
                err.spanned(e.union_token, "Only `struct` types supported");
                TokenStream::new()
            }
        }
    }

    pub fn visit_fields(
        &self,
        data: &syn::DataStruct,
        err: &Errors,
        handler: impl DataHandler,
    ) -> Vec<TokenStream> {
        match data.fields {
            syn::Fields::Named(ref fields) => fields
                .named
                .iter()
                .map(|f| handler.on_field(f, err))
                .collect(),
            _ => {
                err.spanned(data.fields, "Only named fields `struct` is supported");
                Vec::new()
            }
        }
    }
}

/// Visits encoding attrubutes and calls `handler`'s appropriate method, see [EncodingHandler].
struct EncodingVisitor {}

/// Handles different kinds of encoding specified in attrubutes
trait EncodingHandler {
    /// Handles bare type, as if no encoding has been specified.
    fn on_type(&self, ty: &syn::Type, err: &Errors) -> TokenStream;
    /// Handles `Bytes` encoding.
    fn on_bytes(&self, err: &Errors) -> TokenStream;
    /// Handles `String` encoding.
    fn on_string(&self, err: &Errors) -> TokenStream;
    /// Handles `BoundedString(size)` encoding.
    fn on_bounded_string(&self, size: &syn::Expr, err: &Errors) -> TokenStream;
    /// Handles `Sized(size, inner)` encoding.
    fn on_sized(
        &self,
        ty: &syn::Type,
        size: &syn::Expr,
        inner: Option<&syn::Meta>,
        err: &Errors,
    ) -> TokenStream;
}

impl EncodingVisitor {
    fn _modifier<'a>(
        meta_list: &'a syn::MetaList,
        err: &Errors,
    ) -> Result<Option<&'a syn::Meta>, syn::Error> {
        let mut it = meta_list.nested.iter();
        let inner = match it.next() {
            Some(syn::NestedMeta::Meta(m)) => Some(m),
            None => None,
            _ => {
                return Err(attr_error(
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
                return Err(attr_error(
                    &meta_list.path,
                    &meta_list,
                    format!("Expecting string literal as `{}` attribute", param),
                ))
            }
            None => {
                return Err(attr_error(
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
                return Err(attr_error(
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
                return Err(attr_error(
                    &meta_list.path,
                    &meta_list,
                    format!("Expecting string literal as `{}` attribute", param),
                ))
            }
            None => {
                return Err(attr_error(
                    &meta_list.path,
                    &meta_list,
                    format!("Missing `{}` attribute", param),
                ))
            }
        };
        Ok(param)
    }

    fn visit(field: &syn::Field, err: &Errors) -> TokenStream {
        let meta = get_encoding_meta(field.attrs, err);
        self.visit_type_encoding(field.ty, meta)
    }

    fn visit_type_encoding<T: EncodingHandler>(
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

    use crate::{DataHandler, EncodingHandler, EncodingVisitor, Errors, get_encoding_meta, spanned_error, symbol::Symbol};

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
        data_visitor: DataVisitor,
        encoding_visitor: EncodingVisitor,
        input: &syn::DeriveInput,
        name_str: String,
    }

    impl EncodingGenerator {
        fn new(input: &syn::DeriveInput) -> Self {
            let data_visitor = DataVisitor {};
            let encoding_visitor = EncodingVisitor {};
            let name_str = input.ident.to_string();
            EncodingGenerator {
                data_visitor,
                encoding_visitor,
                input,
                name_str,
            }
        }

        fn generate(&self, err: &Errors) -> TokenStream {
            let name = self.input.ident;
            let name_str = self.name_str;
            let encoding_static_name = syn::Ident::new(
                &format!("__TEZOS_ENCODING_{}", name_str.to_uppercase()),
                name.span(),
            );
            let data_encoding = self.data_visitor.visit_data(self.input.data, err, self);
            let encoding = self.encoding_visitor.visit(self.input.attrs, err, self);
            quote! {
                lazy_static::lazy_static! {
                    #[allow(non_upper_case_globals)]
                    static ref #encoding_static_name: tezos_encoding::encoding::Encoding = #data_encoding;
                }

                impl tezos_encoding::encoding::HasEncoding for #name {
                    fn encoding() -> &'static tezos_encoding::encoding::Encoding {
                        &#encoding_static_name
                    }
                }
            }
        }
    }

    impl DataHandler for EncodingGenerator {
        fn on_struct(&self, data: &syn::DataStruct, err: &Errors) -> TokenStream {
            let name = self.name_str;
            let fields_encoding = self.data_visitor.visit_fields(data, err, self);
            quote! {
                tezos_encoding::encoding::Encoding::Obj(
                    #name,
                    vec![
                        #(#fields_encoding),*
                    ]
                )
            }
        }

        fn on_field(&self, field: &syn::Field, err: &Errors) -> TokenStream {
            let field_encoding = self.encoding_visitor.visit_type_attribute(field, err, self);
            let name = field
                .ident
                .as_ref()
                .map(|i| format!("{}", &i))
                .unwrap_or(String::new());
            quote! {
                tezos_encoding::encoding::Field::new(#name, #field_encoding),
            }
        }

        fn on_enum(&self, data: &syn::DataEnum, err: &Errors) -> TokenStream {
            todo!()
        }

        fn on_variant(&self, variant: &syn::Variant, err: &Errors) -> TokenStream {
            todo!()
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

    pub fn struct_encoding(data: &syn::DataStruct, name: &str) -> TokenStream {
        match data.fields {
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
                    tezos_encoding::encoding::Encoding::Obj(
                        #name,
                        vec![
                            #(#recurse)*
                        ]
                    )
                }
            }
            _ => spanned_error(&data.fields, "Only `struct` with named fields supported"),
        }
    }

    fn generate_tag_encoding(variant: &syn::Variant) -> TokenStream {}

    fn enum_encoding(data: &syn::DataEnum) -> TokenStream {
        let tags_encoding = data.variants.iter().map(|v| {
            let tag_encoding = generate_tag_encoding(v);
            quote!(#tag_encoding)
        });
        quote! {
            Encoding::Tags(
                size_of::<xxx>,
                TagMap::new(vec![
                    #(#tags_encoding),*
                ]),
            )
        }
    }

    pub fn data_encoding(data: &syn::Data, name: &str) -> TokenStream {
        match data {
            syn::Data::Struct(data) => struct_encoding(data, name),
            syn::Data::Enum(data) => enum_encoding(data),
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
        let data_encoding = data_encoding(&input.data, &name_str);
        quote! {
            lazy_static::lazy_static! {
                #[allow(non_upper_case_globals)]
                static ref #encoding_static_name: tezos_encoding::encoding::Encoding = #data_encoding;
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

*/
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
