use ff::{PrimeField, PrimeFieldDecodingError, PrimeFieldRepr};
use group::EncodedPoint;
use pairing::bls12_381::{Fq, Fq12, Fq2, Fq6, Fr};
use pairing::bls12_381::{G1Compressed, G1Uncompressed, G2Compressed, G2Uncompressed};

#[cfg(not(feature = "wasm"))]
use libc::c_uchar;

#[cfg(feature = "wasm")]
#[allow(non_camel_case_types)]
#[cfg(feature = "wasm")]
type c_uchar = u8;

use super::{
    LENGTH_COMPRESSED_G1_BYTES, LENGTH_COMPRESSED_G2_BYTES, LENGTH_FQ12_BYTES, LENGTH_FQ_BYTES,
    LENGTH_FR_BYTES, LENGTH_UNCOMPRESSED_G1_BYTES, LENGTH_UNCOMPRESSED_G2_BYTES,
};

// c_uchar and u8 are the same type
pub fn read_uncompressed_g1(from: &[c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES]) -> G1Uncompressed {
    let mut g1 = G1Uncompressed::empty();
    for (d, s) in g1.as_mut().iter_mut().zip(from.iter()) {
        *d = *s;
    }
    g1
}

pub fn read_uncompressed_g2(from: &[c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES]) -> G2Uncompressed {
    let mut g = G2Uncompressed::empty();
    for (d, s) in g.as_mut().iter_mut().zip(from.iter()) {
        *d = *s;
    }
    g
}

pub fn read_compressed_g1(from: &[c_uchar; LENGTH_COMPRESSED_G1_BYTES]) -> G1Compressed {
    let mut g = G1Compressed::empty();
    for (d, s) in g.as_mut().iter_mut().zip(from.iter()) {
        *d = *s;
    }
    g
}

pub fn read_compressed_g2(from: &[c_uchar; LENGTH_COMPRESSED_G2_BYTES]) -> G2Compressed {
    let mut g = G2Compressed::empty();
    for (d, s) in g.as_mut().iter_mut().zip(from.iter()) {
        *d = *s;
    }
    g
}

// We only need 256 bits for fr
pub fn read_fr(from: &[c_uchar; LENGTH_FR_BYTES]) -> Result<Fr, PrimeFieldDecodingError> {
    let mut f = <Fr as PrimeField>::Repr::default();
    f.read_le(&from[..]).unwrap();
    Fr::from_repr(f)
}

// the from contains the two coordinates ==> 2 x 48
pub fn read_fq12(from: &[c_uchar; LENGTH_FQ12_BYTES]) -> Result<Fq12, &'static str> {
    let mut x1 = <Fq as PrimeField>::Repr::default();
    let mut x2 = <Fq as PrimeField>::Repr::default();
    let mut x3 = <Fq as PrimeField>::Repr::default();
    let mut x4 = <Fq as PrimeField>::Repr::default();
    let mut x5 = <Fq as PrimeField>::Repr::default();
    let mut x6 = <Fq as PrimeField>::Repr::default();
    let mut x7 = <Fq as PrimeField>::Repr::default();
    let mut x8 = <Fq as PrimeField>::Repr::default();
    let mut x9 = <Fq as PrimeField>::Repr::default();
    let mut x10 = <Fq as PrimeField>::Repr::default();
    let mut x11 = <Fq as PrimeField>::Repr::default();
    let mut x12 = <Fq as PrimeField>::Repr::default();
    x1.read_le(&from[..LENGTH_FQ_BYTES]).unwrap();
    x2.read_le(&from[LENGTH_FQ_BYTES..LENGTH_FQ_BYTES * 2])
        .unwrap();
    x3.read_le(&from[LENGTH_FQ_BYTES * 2..LENGTH_FQ_BYTES * 3])
        .unwrap();
    x4.read_le(&from[LENGTH_FQ_BYTES * 3..LENGTH_FQ_BYTES * 4])
        .unwrap();
    x5.read_le(&from[LENGTH_FQ_BYTES * 4..LENGTH_FQ_BYTES * 5])
        .unwrap();
    x6.read_le(&from[LENGTH_FQ_BYTES * 5..LENGTH_FQ_BYTES * 6])
        .unwrap();
    x7.read_le(&from[LENGTH_FQ_BYTES * 6..LENGTH_FQ_BYTES * 7])
        .unwrap();
    x8.read_le(&from[LENGTH_FQ_BYTES * 7..LENGTH_FQ_BYTES * 8])
        .unwrap();
    x9.read_le(&from[LENGTH_FQ_BYTES * 8..LENGTH_FQ_BYTES * 9])
        .unwrap();
    x10.read_le(&from[LENGTH_FQ_BYTES * 9..LENGTH_FQ_BYTES * 10])
        .unwrap();
    x11.read_le(&from[LENGTH_FQ_BYTES * 10..LENGTH_FQ_BYTES * 11])
        .unwrap();
    x12.read_le(&from[LENGTH_FQ_BYTES * 11..LENGTH_FQ12_BYTES])
        .unwrap();
    let x1 = Fq::from_repr(x1);
    let x2 = Fq::from_repr(x2);
    let x3 = Fq::from_repr(x3);
    let x4 = Fq::from_repr(x4);
    let x5 = Fq::from_repr(x5);
    let x6 = Fq::from_repr(x6);
    let x7 = Fq::from_repr(x7);
    let x8 = Fq::from_repr(x8);
    let x9 = Fq::from_repr(x9);
    let x10 = Fq::from_repr(x10);
    let x11 = Fq::from_repr(x11);
    let x12 = Fq::from_repr(x12);
    // PLEASE IMPROVEME
    match (x1, x2, x3, x4, x5, x6, x7, x8, x9, x10, x11, x12) {
        (
            Ok(x1),
            Ok(x2),
            Ok(x3),
            Ok(x4),
            Ok(x5),
            Ok(x6),
            Ok(x7),
            Ok(x8),
            Ok(x9),
            Ok(x10),
            Ok(x11),
            Ok(x12),
        ) => Ok(Fq12 {
            c0: Fq6 {
                c0: Fq2 { c0: x1, c1: x2 },
                c1: Fq2 { c0: x3, c1: x4 },
                c2: Fq2 { c0: x5, c1: x6 },
            },
            c1: Fq6 {
                c0: Fq2 { c0: x7, c1: x8 },
                c1: Fq2 { c0: x9, c1: x10 },
                c2: Fq2 { c0: x11, c1: x12 },
            },
        }),
        _ => Err("One of the element is not in the field"),
    }
}

pub fn read_fq(from: &[c_uchar; LENGTH_FQ_BYTES]) -> Result<Fq, PrimeFieldDecodingError> {
    let mut f = <Fq as PrimeField>::Repr::default();
    f.read_le(&from[..]).unwrap();
    Fq::from_repr(f)
}
