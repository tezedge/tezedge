//! Copyright (c) SimpleStaking and Tezedge Contributors
//! SPDX-License-Identifier: MIT

use std::convert::TryFrom;
use std::fmt;

pub use tezos_encoding_derive::BinWriter;

use thiserror::Error;

#[derive(Debug, Error)]
/// Encoding error kind.
pub enum BinErrorKind {
    /// I/O Error.
    #[error("I/O error: {0}")]
    IOError(std::io::Error),
    /// Boundary violation error, contains expected and actual sizes.
    #[error("Boundary violation: expected {0}, got {1}")]
    SizeError(usize, usize),
    /// Field which encoding caused an error.
    #[error("Error encoding field: {0}")]
    FieldError(&'static str),
    /// Enum variant which encoding caused an error.
    #[error("Error encoding enum variant: {0}")]
    VariantError(&'static str),
    /// Other error.
    #[error("Other error: {0}")]
    CustomError(String),
}

#[derive(Debug)]
pub struct BinError(Vec<BinErrorKind>);

impl std::error::Error for BinError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl From<BinErrorKind> for BinError {
    fn from(kind: BinErrorKind) -> Self {
        Self(vec![kind])
    }
}

impl BinError {
    fn size_error(expected: usize, actual: usize) -> Self {
        BinErrorKind::SizeError(expected, actual).into()
    }

    pub fn custom(message: String) -> Self {
        BinErrorKind::CustomError(message).into()
    }

    fn field(mut self, name: &'static str) -> Self {
        self.0.push(BinErrorKind::FieldError(name));
        self
    }

    fn variant(mut self, name: &'static str) -> Self {
        self.0.push(BinErrorKind::VariantError(name));
        self
    }

    pub fn iter(&self) -> impl Iterator<Item = &BinErrorKind> {
        self.0.iter()
    }
}

impl fmt::Display for BinError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut first = true;
        for kind in self.0.iter() {
            if !first {
                write!(f, " in context of: ")?;
            }
            first = false;
            write!(f, "{}", kind)?;
        }
        Ok(())
    }
}

impl From<std::io::Error> for BinError {
    fn from(error: std::io::Error) -> Self {
        BinErrorKind::IOError(error).into()
    }
}

pub struct AndThen<F, G, D1, D2> {
    f: F,
    g: G,
    phantom: core::marker::PhantomData<(D1, D2)>,
}

impl<F: BinSerializer<D1>, G: BinSerializer<D2>, D1, D2> BinSerializer<(D1, D2)>
    for AndThen<F, G, D1, D2>
{
    fn serialize(&mut self, (d1, d2): (D1, D2), out: &mut Vec<u8>) -> BinResult {
        self.f.serialize(d1, out)?;
        self.g.serialize(d2, out)?;
        Ok(())
    }
}

pub struct AddError<F, G, D> {
    f: F,
    g: G,
    phantom: core::marker::PhantomData<D>,
}

impl<F, G, D> BinSerializer<D> for AddError<F, G, D>
where
    F: BinSerializer<D>,
    G: FnMut(BinError) -> BinError,
{
    fn serialize(&mut self, data: D, out: &mut Vec<u8>) -> BinResult {
        self.f.serialize(data, out).map_err(move |e| (self.g)(e))
    }
}

pub trait BinSerializer<D> {
    fn serialize(&mut self, data: D, out: &mut Vec<u8>) -> BinResult;

    fn and_then<U, G>(self, g: G) -> AndThen<Self, G, D, U>
    where
        G: BinSerializer<U>,
        Self: Sized,
    {
        AndThen {
            f: self,
            g,
            phantom: core::marker::PhantomData,
        }
    }

    fn add_error<G>(self, g: G) -> AddError<Self, G, D>
    where
        G: FnMut(BinError) -> BinError,
        Self: Sized,
    {
        AddError {
            f: self,
            g,
            phantom: core::marker::PhantomData,
        }
    }
}

impl<T, F> BinSerializer<T> for F
where
    T: Sized,
    F: FnMut(T, &mut Vec<u8>) -> BinResult,
{
    fn serialize(&mut self, data: T, out: &mut Vec<u8>) -> BinResult {
        self(data, out)
    }
}

pub type BinResult = Result<(), BinError>;

pub trait BinWriter {
    fn bin_write(&self, bytes: &mut Vec<u8>) -> BinResult;
}

impl BinWriter for u16 {
    fn bin_write(&self, out: &mut Vec<u8>) -> BinResult {
        put_bytes(&self.to_be_bytes(), out);
        Ok(())
    }
}

pub fn put_bytes(bytes: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(bytes);
}

pub fn put_byte(byte: &u8, out: &mut Vec<u8>) {
    out.push(*byte)
}

fn put_size(size: usize, out: &mut Vec<u8>) -> BinResult {
    let size =
        u32::try_from(size).map_err(|_| BinError::size_error((u32::MAX >> 2) as usize, size))?;
    put_bytes(&size.to_be_bytes(), out);
    Ok(())
}

fn put_short_size(size: usize, out: &mut Vec<u8>) -> BinResult {
    let size = u8::try_from(size).map_err(|_| BinError::size_error(u8::MAX as usize, size))?;
    put_bytes(&size.to_be_bytes(), out);
    Ok(())
}

pub fn bytes<T: AsRef<[u8]>>(bytes: T, out: &mut Vec<u8>) -> BinResult {
    out.extend_from_slice(bytes.as_ref());
    Ok(())
}

pub fn boolean(b: &bool, out: &mut Vec<u8>) -> BinResult {
    put_byte(
        if *b {
            &crate::types::BYTE_VAL_TRUE
        } else {
            &crate::types::BYTE_VAL_FALSE
        },
        out,
    );
    Ok(())
}

// Rust integers encoding
mod integers {
    macro_rules! encode_integer {
        ($t:ident) => {
            pub fn $t(i: &$t, out: &mut Vec<u8>) -> super::BinResult {
                super::put_bytes(&i.to_be_bytes(), out);
                Ok(())
            }
        };
    }

    encode_integer!(i8);
    encode_integer!(i16);
    encode_integer!(i32);
    encode_integer!(i64);
    encode_integer!(u8);
    encode_integer!(u16);
    encode_integer!(u32);
    encode_integer!(u64);
}

pub use integers::*;

macro_rules! encode_hash {
    ($hash_name:ty) => {
        impl BinWriter for $hash_name {
            fn bin_write(&self, out: &mut Vec<u8>) -> BinResult {
                put_bytes(self.as_ref(), out);
                Ok(())
            }
        }
    };
}

encode_hash!(crypto::hash::ChainId);
encode_hash!(crypto::hash::BlockHash);
encode_hash!(crypto::hash::BlockMetadataHash);
encode_hash!(crypto::hash::BlockPayloadHash);
encode_hash!(crypto::hash::OperationHash);
encode_hash!(crypto::hash::OperationListListHash);
encode_hash!(crypto::hash::OperationMetadataHash);
encode_hash!(crypto::hash::OperationMetadataListListHash);
encode_hash!(crypto::hash::ContextHash);
encode_hash!(crypto::hash::ProtocolHash);
encode_hash!(crypto::hash::ContractKt1Hash);
encode_hash!(crypto::hash::ContractTz1Hash);
encode_hash!(crypto::hash::ContractTz2Hash);
encode_hash!(crypto::hash::ContractTz3Hash);
encode_hash!(crypto::hash::CryptoboxPublicKeyHash);
encode_hash!(crypto::hash::PublicKeyEd25519);
encode_hash!(crypto::hash::PublicKeySecp256k1);
encode_hash!(crypto::hash::PublicKeyP256);
encode_hash!(crypto::hash::Signature);

pub fn sized<T>(
    size: usize,
    mut serializer: impl BinSerializer<T>,
) -> impl FnMut(T, &mut Vec<u8>) -> BinResult {
    move |data, out| {
        let len = out.len();
        serializer.serialize(data, out)?;
        if out.len() - len != size {
            Err(BinError::size_error(size, out.len() - len))
        } else {
            Ok(())
        }
    }
}

pub fn string(data: impl AsRef<str>, out: &mut Vec<u8>) -> BinResult {
    put_size(data.as_ref().len(), out)?;
    put_bytes(data.as_ref().as_bytes(), out);
    Ok(())
}

pub fn bounded_string<S: AsRef<str>>(max_len: usize) -> impl FnMut(S, &mut Vec<u8>) -> BinResult {
    move |data, out| {
        if data.as_ref().len() <= max_len {
            string(data, out)
        } else {
            Err(BinError::size_error(max_len, data.as_ref().len()))
        }
    }
}

pub fn list<T: IntoIterator>(
    mut serializer: impl BinSerializer<T::Item>,
) -> impl FnMut(T, &mut Vec<u8>) -> BinResult {
    move |data, out| {
        data.into_iter()
            .try_for_each(|item| serializer.serialize(item, out))
    }
}

pub fn bounded_list<T: IntoIterator>(
    max_len: usize,
    mut serializer: impl BinSerializer<T::Item>,
) -> impl FnMut(T, &mut Vec<u8>) -> BinResult {
    move |data, out| {
        let iter = data.into_iter();
        if iter.size_hint().0 > max_len {
            return Err(BinError::size_error(max_len, iter.size_hint().0));
        }
        iter.enumerate().try_for_each(|(i, item)| {
            if i > max_len {
                Err(BinError::size_error(max_len, i))
            } else {
                serializer.serialize(item, out)
            }
        })
    }
}

pub fn bounded<T>(
    max_size: usize,
    mut serializer: impl BinSerializer<T>,
) -> impl FnMut(T, &mut Vec<u8>) -> BinResult {
    move |data, out| {
        let size = out.len();
        serializer.serialize(data, out)?;
        if out.len() - size > max_size {
            Err(BinError::size_error(max_size, out.len() - size))
        } else {
            Ok(())
        }
    }
}

pub fn dynamic<T>(
    mut serializer: impl BinSerializer<T>,
) -> impl FnMut(T, &mut Vec<u8>) -> BinResult {
    move |data, out| {
        let mut tmp_out = Vec::new();
        serializer.serialize(data, &mut tmp_out)?;
        put_size(tmp_out.len(), out)?;
        out.extend(tmp_out);
        Ok(())
    }
}

pub fn short_dynamic<T>(
    mut serializer: impl BinSerializer<T>,
) -> impl FnMut(T, &mut Vec<u8>) -> BinResult {
    move |data, out| {
        let mut tmp_out = Vec::new();
        serializer.serialize(data, &mut tmp_out)?;
        put_short_size(tmp_out.len(), out)?;
        out.extend(tmp_out);
        Ok(())
    }
}

pub fn bounded_dynamic<T>(
    max_size: usize,
    mut serializer: impl BinSerializer<T>,
) -> impl FnMut(T, &mut Vec<u8>) -> BinResult {
    move |data, out| {
        let mut tmp_out = Vec::new();
        serializer.serialize(data, &mut tmp_out)?;
        if tmp_out.len() > max_size {
            Err(BinError::size_error(max_size, tmp_out.len()))
        } else {
            put_size(tmp_out.len(), out)?;
            out.extend(tmp_out);
            Ok(())
        }
    }
}

pub fn field<D>(
    name: &'static str,
    serializer: impl BinSerializer<D>,
) -> impl FnMut(D, &mut Vec<u8>) -> BinResult {
    let mut serializer = serializer.add_error(move |e| e.field(name));
    move |data, out| serializer.serialize(data, out)
}

pub fn variant<D>(
    name: &'static str,
    tag: impl BinSerializer<D>,
) -> impl FnMut(D, &mut Vec<u8>) -> BinResult {
    let mut serializer = tag.add_error(move |e| e.variant(name));
    move |data, out| serializer.serialize(data, out)
}

pub fn variant_with_field<D1, D2>(
    name: &'static str,
    tag: impl BinSerializer<D1>,
    field: impl BinSerializer<D2>,
) -> impl FnMut(D1, D2, &mut Vec<u8>) -> BinResult {
    let mut serializer = tag.and_then(field).add_error(move |e| e.variant(name));
    move |tag, field, out| serializer.serialize((tag, field), out)
}

pub fn optional_field<'a, T: 'a>(
    mut f: impl BinSerializer<&'a T>,
) -> impl FnMut(&'a Option<T>, &mut Vec<u8>) -> BinResult {
    move |opt, out| {
        match opt.as_ref() {
            Some(field) => {
                put_byte(&crate::types::BYTE_FIELD_SOME, out);
                f.serialize(field, out)?;
            }
            None => {
                put_byte(&crate::types::BYTE_FIELD_NONE, out);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::BinResult;

    fn serialize_slice(slice: &[u8], out: &mut Vec<u8>) -> BinResult {
        out.extend_from_slice(slice);
        Ok(())
    }

    fn serialize_u16(n: &u16, out: &mut Vec<u8>) -> BinResult {
        out.extend_from_slice(&n.to_be_bytes());
        Ok(())
    }

    #[test]
    fn bytes() {
        let mut out = Vec::new();
        super::bytes(&[1, 2, 3], &mut out).expect("Should not fail");
        assert_eq!(&out, &[1, 2, 3]);
    }

    #[test]
    fn u8() {
        let out = &mut Vec::new();
        super::u8(&0, out).expect("Should not fail");
    }

    #[test]
    fn u16() {
        let out = &mut Vec::new();
        super::u16(&0, out).expect("Should not fail");
    }

    #[test]
    fn sized() {
        let mut out = Vec::new();
        super::sized(3, super::bytes)(&[1, 2, 3], &mut out).expect("Should not fail");
        assert_eq!(&out, &[1, 2, 3]);

        let mut out = Vec::new();
        super::sized(2, super::bytes)(&[1, 2, 3], &mut out).expect_err("Should fail");

        let mut out = Vec::new();
        super::sized(4, super::bytes)(&[1, 2, 3], &mut out).expect_err("Should fail");
    }

    #[test]
    fn string() {
        let mut out = Vec::new();
        super::string("abc", &mut out).expect("Should not fail");
        assert_eq!(&out, &[0, 0, 0, 3, 97, 98, 99]);
    }

    #[test]
    fn bounded_string() {
        let mut out = Vec::new();
        super::bounded_string(3)("abc", &mut out).expect("Should not fail");
        assert_eq!(&out, &[0, 0, 0, 3, 97, 98, 99]);

        let mut out = Vec::new();
        super::bounded_string(2)("abc", &mut out).expect_err("Should fail");
    }

    #[test]
    fn list() {
        let mut out = Vec::new();
        super::list(serialize_u16)(&[1, 2, 3], &mut out).expect("Should not fail");
        assert_eq!(&out, &[0, 1, 0, 2, 0, 3]);
    }

    #[test]
    fn bounded_list() {
        let mut out = Vec::new();
        super::bounded_list(3, serialize_u16)(&[1, 2, 3], &mut out).expect("Should not fail");
        assert_eq!(&out, &[0, 1, 0, 2, 0, 3]);

        let mut out = Vec::new();
        super::bounded_list(2, serialize_u16)(&[1, 2, 3], &mut out).expect_err("Should fail");
    }

    #[test]
    fn bounded() {
        let mut out = Vec::new();
        super::bounded(3, serialize_slice)(&[1, 2, 3], &mut out).expect("Should not fail");
        assert_eq!(&out, &[1, 2, 3]);

        let mut out = Vec::new();
        super::bounded(2, serialize_slice)(&[1, 2, 3], &mut out).expect_err("Should fail");
    }

    #[test]
    fn short_dynamic() {
        let mut out = Vec::new();
        super::short_dynamic(serialize_slice)(&[1, 2, 3], &mut out).expect("Should not fail");
        assert_eq!(out[0], 3);
        assert_eq!(&out[1..], &[1, 2, 3]);

        let mut out = Vec::new();
        super::short_dynamic(serialize_slice)(&[0; 257], &mut out).expect_err("Should fail");
    }

    #[test]
    fn dynamic() {
        let mut out = Vec::new();
        super::dynamic(serialize_slice)(&[1, 2, 3], &mut out).expect("Should not fail");
        assert_eq!(&out[0..4], &[0, 0, 0, 3]);
        assert_eq!(&out[4..], &[1, 2, 3]);
    }

    #[test]
    fn bounded_dynamic() {
        let mut out = Vec::new();
        super::bounded_dynamic(10, serialize_slice)(&[1, 2, 3], &mut out).expect("Should not fail");
        assert_eq!(&out[0..4], &[0, 0, 0, 3]);
        assert_eq!(&out[4..], &[1, 2, 3]);

        let mut out = Vec::new();
        super::bounded_dynamic(2, serialize_slice)(&[1, 2, 3], &mut out).expect_err("Should fail");
    }

    #[test]
    fn field() {
        let mut out = Vec::new();
        super::field("field", super::u8)(&10, &mut out).expect("Should not fail");

        let mut out = Vec::new();
        let field_ser = super::bounded_dynamic(2, serialize_slice); // 2 bytes max
        let err = super::field("field", field_ser)(&[1, 2, 3], &mut out).expect_err("Should fail");
        assert!(matches!(
            err.iter().last().unwrap(),
            super::BinErrorKind::FieldError(..),
        ));
    }

    #[test]
    fn variant() {
        let mut out = Vec::new();
        super::variant("variant", super::u8)(&10, &mut out).expect("Should not fail");

        let mut out = Vec::new();
        let variant_ser = super::bounded_dynamic(2, serialize_slice); // 2 bytes max
        let err =
            super::variant("variant", variant_ser)(&[1, 2, 3], &mut out).expect_err("Should fail");
        assert!(matches!(
            err.iter().last().unwrap(),
            super::BinErrorKind::VariantError(..),
        ));
    }

    #[test]
    fn variant_with_field() {
        let mut out = Vec::new();
        super::variant_with_field("variant_with_field", super::u8, super::u16)(&10, &30, &mut out)
            .expect("Should not fail");

        let mut out = Vec::new();
        let variant_with_field_ser = super::bounded_dynamic(2, serialize_slice); // 2 bytes max
        let err = super::variant_with_field(
            "variant_with_field",
            super::u8,
            variant_with_field_ser,
        )(&0, &[1, 2, 3], &mut out)
        .expect_err("Should fail");
        assert!(matches!(
            err.iter().last().unwrap(),
            super::BinErrorKind::VariantError(..),
        ));
    }

    /// Test on compilation error for optional vec field.
    #[test]
    fn optional_sized_bytes() {
        let data: Option<Vec<u8>> = Some(vec![0; 32]);
        let mut out = Vec::new();
        super::optional_field(super::sized(32, super::bytes))(&data, &mut out)
            .expect("Should not fail");
    }
}
