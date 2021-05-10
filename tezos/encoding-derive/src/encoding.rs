use proc_macro2::Span;


#[derive(Debug)]
pub struct DataWithEncoding<'a> {
    pub name: &'a syn::Ident,
    pub encoding: Encoding<'a>,
}

#[derive(Debug)]
pub struct StructEncoding<'a> {
    pub name: &'a syn::Ident,
    pub fields: Vec<FieldEncoding<'a>>,
}

#[derive(Debug)]
pub struct FieldEncoding<'a> {
    pub name: &'a syn::Ident,
    pub encoding: Option<Encoding<'a>>,
}

#[derive(Debug)]
pub struct EnumEncoding<'a> {
    pub name: &'a syn::Ident,
    pub tag_type: syn::Ident,
    pub tags: Vec<Tag<'a>>,
}

#[derive(Debug)]
pub struct Tag<'a> {
    pub id: syn::LitInt,
    pub name: &'a syn::Ident,
    pub encoding: Encoding<'a>,
}

#[derive(Debug)]
pub enum Encoding<'a> {
    Unit,
    Primitive(&'a syn::Ident),
    Bytes(Span),
    Path(&'a syn::Path),
    Struct(StructEncoding<'a>),
    Enum(EnumEncoding<'a>),
    String(Option<syn::Expr>, Span),
    List(Option<syn::Expr>, Box<Encoding<'a>>, Span),
    Sized(syn::Expr, Box<Encoding<'a>>, Span),
    Bounded(syn::Expr, Box<Encoding<'a>>, Span),
    Dynamic(Option<syn::Expr>, Box<Encoding<'a>>, Span),
}
