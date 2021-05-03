extern crate proc_macro;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Lit, Meta, NestedMeta, Type};

mod symbol;

/// Checks that the type is `Vec<u8>`
fn is_vec_u8(_ty: &syn::Type) -> bool {
    // TODO
    true
}

/// Finds `encoding` attribute and parse its content
fn get_encoding_meta(attrs: &[syn::Attribute]) -> Option<syn::Meta> {
    attrs.iter().find_map(|attr| {
        if attr.path == symbol::ENCODING {
            let meta = attr.parse_meta().unwrap();
            match meta {
                syn::Meta::List(m) => {
                    assert!(m.nested.len() == 1);
                    let meta = m.nested.into_iter().next();
                    match meta {
                        Some(syn::NestedMeta::Meta(m)) => Some(m),
                        _ => panic!("Wrong parameter for `encoding` attribute: {:?}", meta),
                    }
                }
                _ => panic!("Unexpected kind of `encoding` attrubute: {:?}", meta),
            }
        } else {
            None
        }
    })
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

/// Visits encoding attrubutes and calls `handler`'s appropriate method, see [EncodingHandler].
struct EncodingVisitor {}

impl EncodingVisitor {
    pub fn visit<T: EncodingHandler>(
        &self,
        ty: &Type,
        meta: Option<&Meta>,
        handler: &T,
    ) -> TokenStream {
        match meta {
            // `Sized("Expr")`, `Sized("Expr", Inner)`
            Some(syn::Meta::List(m)) if m.path == symbol::SIZED => {
                let mut it = m.nested.iter();
                let size = it.next().unwrap();
                let size = match size {
                    syn::NestedMeta::Lit(syn::Lit::Str(s)) => syn::parse_str(&s.value()).unwrap(),
                    _ => panic!("Wrong size parameter for `Sized` attribute: {:?}", size),
                };
                let inner = it.next().map(|inner| match inner {
                    syn::NestedMeta::Meta(m) => m,
                    _ => panic!("Wrong inner parameter for `Sized` attribute: {:?}", inner),
                });
                handler.on_sized(ty, &size, inner)
            }
            // `Bytes`
            Some(syn::Meta::Path(p)) if p == symbol::BYTES => {
                assert!(is_vec_u8(ty));
                handler.on_bytes()
            }
            // `BoundedString("SIZE")`
            Some(Meta::List(m)) if m.path == symbol::BOUNDED_STRING => {
                let mut it = m.nested.iter();
                let size = it.next().unwrap();
                let size = match size {
                    NestedMeta::Lit(Lit::Str(s)) => syn::parse_str(&s.value()).unwrap(),
                    _ => panic!(
                        "Wrong size parameter for `BoundedString` attribute: {:?}",
                        size
                    ),
                };
                assert!(is_vec_u8(ty));
                handler.on_bounded_string(&size)
            }
            // no attributes, use type information
            None => handler.on_type(ty),
            _ => panic!("Encoding not implemented for attribute {:?}", meta),
        }
    }
}

mod enc {
    use lazy_static::lazy_static;
    use proc_macro2::TokenStream;
    use quote::quote;
    use syn::spanned::Spanned;

    use crate::{get_encoding_meta, symbol::Symbol, EncodingHandler, EncodingVisitor};

    lazy_static! {
        static ref DIRECT_MAPPING: std::collections::HashMap<Symbol, &'static str> = {
            use crate::symbol::*;
            [(U8, "Uint8"), (U16, "Uint16")].iter().cloned().collect()
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
                _ => panic!("Encoding not implemented for type {:?}", ty),
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
        let meta = get_encoding_meta(&field.attrs);
        EncodingGenerator::new().generate(&field.ty, meta.as_ref())
    }

    pub fn struct_fields_encoding(data: &syn::Data) -> TokenStream {
        match *data {
            syn::Data::Struct(ref data) => match data.fields {
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
                _ => panic!("Only `struct` with named fields supported"),
            },
            _ => panic!("Only `struct` types supported"),
        }
    }
}

#[proc_macro_derive(HasEncoding, attributes(encoding))]
pub fn derive_tezos_encoding(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let name_str = name.to_string();
    let encoding_static_name = syn::Ident::new(
        &format!("__TEZOS_ENCODING_{}", name.to_string().to_uppercase()),
        name.span(),
    );
    let fields_encoding = enc::struct_fields_encoding(&input.data);

    let expanded = quote! {
        lazy_static::lazy_static! {
            #[allow(non_upper_case_globals)]
            static ref #encoding_static_name: tezos_encoding::encoding::Encoding = tezos_encoding::encoding::Encoding::Obj(
                #name_str,
                vec![
                    #fields_encoding
                ]
            );
        }

        impl tezos_encoding::encoding::HasEncodingDerived for #name {
            fn encoding_derived() -> &'static tezos_encoding::encoding::Encoding {
                &#encoding_static_name
            }
        }

        impl tezos_encoding::encoding::HasEncoding for #name {
            fn encoding() -> &'static tezos_encoding::encoding::Encoding {
                &#encoding_static_name
            }
        }

        impl crate::p2p::binary_message::BinaryMessage for #name {
            #[inline]
            fn as_bytes(&self) -> Result<Vec<u8>, tezos_encoding::binary_writer::BinaryWriterError> {
                // if cache not configured or empty, resolve by encoding
                tezos_encoding::binary_writer::write(self, &Self::encoding_derived())
            }

            #[inline]
            fn from_bytes<B: AsRef<[u8]>>(bytes: B) -> Result<Self, tezos_encoding::binary_reader::BinaryReaderError> {
                let (bytes, myself) = Self::from_bytes_nom(bytes.as_ref())?;
                Ok(myself)
            }
        }
    };
    proc_macro::TokenStream::from(expanded)
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
