extern crate proc_macro;

use lazy_static::lazy_static;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, spanned::Spanned, Data, DeriveInput, Expr, Field, Fields, Lit,
    Meta, NestedMeta, Path, Type,
};

mod symbol;

lazy_static! {
    static ref DIRECT_MAPPING: std::collections::HashMap<String, &'static str> =
        [("u8".to_string(), "Uint8"), ("u16".to_string(), "Uint16"),]
            .iter()
            .cloned()
            .collect();
}

fn is_vec_u8(_ty: &Type) -> bool {
    // TODO
    true
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

fn direct_mapping(path: &Path) -> Option<TokenStream> {
    let len = path.segments.len();
    if len > 2 {
        return None;
    }
    let mut it = path.segments.iter();
    if len == 2 && it.next().unwrap().ident != "std" {
        return None;
    }
    DIRECT_MAPPING
        .get(&it.next().unwrap().ident.to_string())
        .map(|s| {
            let ident = syn::Ident::new(&format!("{}", s), path.span());
            quote! {
                tezos_encoding::encoding::Encoding::#ident
            }
        })
}

/*
fn type_encoding(ty: &Type, attrs: &Option<Meta>) -> TokenStream {
    match ty {
        Type::Path(ref path) => {
            if let Some(attrs) = attrs {

            } else {
                if let Some(tokens) = direct_mapping(&path.path) {
                    return tokens.into();
                }
                quote! { #ty::encoding().clone() }
            }
        }
        _ => unimplemented!(),
    }
}
*/

fn generate_encoding_sized(ty: &Type, size: &Expr, inner: Option<&Meta>) -> TokenStream {
    let inner = generate_encoding(ty, inner);
    quote! {
        tezos_encoding::encoding::Encoding::Sized(#size, Box::new(#inner))
    }
}

fn _generate_encoding_string(ty: &Type) -> TokenStream {
    assert!(is_vec_u8(ty));
    quote! {
        tezos_encoding::encoding::Encoding::String
    }
}

fn generate_encoding_bounded_string(ty: &Type, size: &Expr) -> TokenStream {
    assert!(is_vec_u8(ty));
    quote! {
        tezos_encoding::encoding::Encoding::BoundedString(#size)
    }
}

fn generate_encoding_bytes(ty: &Type) -> TokenStream {
    assert!(is_vec_u8(ty));
    quote! { tezos_encoding::encoding::Encoding::Bytes }
}

fn generate_encoding(ty: &Type, meta: Option<&Meta>) -> TokenStream {
    match meta {
        // `Sized("Expr")`, `Sized("Expr", Inner)`
        Some(Meta::List(m)) if m.path == symbol::SIZED => {
            let mut it = m.nested.iter();
            let size = it.next().unwrap();
            let size = match size {
                NestedMeta::Lit(Lit::Str(s)) => syn::parse_str(&s.value()).unwrap(),
                _ => panic!("Wrong size parameter for `Sized` attribute: {:?}", size),
            };
            let inner = it.next().map(|inner| match inner {
                NestedMeta::Meta(m) => m,
                _ => panic!("Wrong inner parameter for `Sized` attribute: {:?}", inner),
            });
            generate_encoding_sized(ty, &size, inner)
        }
        // `Bytes`
        Some(Meta::Path(p)) if p == symbol::BYTES => {
            generate_encoding_bytes(ty)
        }
        // `BoundedString("SIZE")`
        Some(Meta::List(m)) if m.path == symbol::BOUNDED_STRING => {
            let mut it = m.nested.iter();
            let size = it.next().unwrap();
            let size = match size {
                NestedMeta::Lit(Lit::Str(s)) => syn::parse_str(&s.value()).unwrap(),
                _ => panic!("Wrong size parameter for `BoundedString` attribute: {:?}", size),
            };
            generate_encoding_bounded_string(ty, &size)
        }
        // no attributes, use type information
        None => match ty {
            Type::Path(path) => {
                if let Some(tokens) = direct_mapping(&path.path) {
                    tokens.into()
                } else {
                    quote! { #ty::encoding().clone() }
                }
            }
            _ => panic!("Encoding not implemented for type {:?}", ty),
        },
        _ => panic!("Encoding not implemented for attribute {:?}", meta),
    }
}

fn generate_field_encoding(field: &Field) -> TokenStream {
    let meta = field.attrs.iter().find_map(|attr| {
        if attr.path == symbol::ENCODING {
            let meta = attr.parse_meta().unwrap();
            match meta {
                Meta::List(m) => {
                    assert!(m.nested.len() == 1);
                    let meta = m.nested.into_iter().next();
                    match meta {
                        Some(NestedMeta::Meta(m)) => Some(m),
                        _ => panic!("Wrong parameter for `encoding` attribute: {:?}", meta),
                    }
                }
                _ => panic!("Unexpected kind of `encoding` attrubute: {:?}", meta),
            }
        } else {
            None
        }
    });
    generate_encoding(&field.ty, meta.as_ref())
}

fn struct_fields_encoding(data: &syn::Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => {
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

#[proc_macro_derive(HasEncoding, attributes(encoding))]
pub fn derive_tezos_encoding(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let name_str = name.to_string();
    let encoding_static_name = syn::Ident::new(
        &format!("__TEZOS_ENCODING_{}", name.to_string().to_uppercase()),
        name.span(),
    );
    let fields_encoding = struct_fields_encoding(&input.data);

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

        impl tezos_encoding::encoding::HasEncoding for #name {
            fn encoding() -> &'static tezos_encoding::encoding::Encoding {
                &#encoding_static_name
            }
        }
    };
    proc_macro::TokenStream::from(expanded)
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
