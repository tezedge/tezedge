use ff::{PrimeField, PrimeFieldRepr};
use pairing::bls12_381::{Fq12, Fr};
use pairing::bls12_381::{G1Compressed, G1Uncompressed, G2Compressed, G2Uncompressed};

#[cfg(not(feature = "wasm"))]
use libc::c_uchar;

#[cfg(feature = "wasm")]
#[allow(non_camel_case_types)]
#[cfg(feature = "wasm")]
type c_uchar = u8;

use super::{
    LENGTH_COMPRESSED_G1_BYTES, LENGTH_COMPRESSED_G2_BYTES, LENGTH_FQ12_BYTES, LENGTH_FR_BYTES,
    LENGTH_UNCOMPRESSED_G1_BYTES, LENGTH_UNCOMPRESSED_G2_BYTES,
};

pub fn write_fr(buffer: *mut [c_uchar; LENGTH_FR_BYTES], element: Fr) {
    let buffer = unsafe { &mut *buffer };
    element.into_repr().write_le(&mut buffer[..]).unwrap();
}

pub fn write_compressed_g1(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
    point: G1Compressed,
) {
    let buffer = unsafe { &mut *buffer };
    for (d, s) in buffer.iter_mut().zip(point.as_ref().iter()) {
        *d = *s;
    }
}

pub fn write_compressed_g2(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
    point: G2Compressed,
) {
    let buffer = unsafe { &mut *buffer };
    for (d, s) in buffer.iter_mut().zip(point.as_ref().iter()) {
        *d = *s;
    }
}

pub fn write_uncompressed_g1(
    buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    point: G1Uncompressed,
) {
    let buffer = unsafe { &mut *buffer };
    for (d, s) in buffer.iter_mut().zip(point.as_ref().iter()) {
        *d = *s;
    }
}

pub fn write_uncompressed_g2(
    buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    point: G2Uncompressed,
) {
    let buffer = unsafe { &mut *buffer };
    for (d, s) in buffer.iter_mut().zip(point.as_ref().iter()) {
        *d = *s;
    }
}

pub fn write_fq12(buffer: *mut [c_uchar; LENGTH_FQ12_BYTES], element: Fq12) {
    let buffer = unsafe { &mut *buffer };
    element
        .c0
        .c0
        .c0
        .into_repr()
        .write_le(&mut buffer[..48])
        .unwrap();
    element
        .c0
        .c0
        .c1
        .into_repr()
        .write_le(&mut buffer[48..96])
        .unwrap();
    element
        .c0
        .c1
        .c0
        .into_repr()
        .write_le(&mut buffer[96..144])
        .unwrap();
    element
        .c0
        .c1
        .c1
        .into_repr()
        .write_le(&mut buffer[144..192])
        .unwrap();
    element
        .c0
        .c2
        .c0
        .into_repr()
        .write_le(&mut buffer[192..240])
        .unwrap();
    element
        .c0
        .c2
        .c1
        .into_repr()
        .write_le(&mut buffer[240..288])
        .unwrap();
    element
        .c1
        .c0
        .c0
        .into_repr()
        .write_le(&mut buffer[288..336])
        .unwrap();
    element
        .c1
        .c0
        .c1
        .into_repr()
        .write_le(&mut buffer[336..384])
        .unwrap();
    element
        .c1
        .c1
        .c0
        .into_repr()
        .write_le(&mut buffer[384..432])
        .unwrap();
    element
        .c1
        .c1
        .c1
        .into_repr()
        .write_le(&mut buffer[432..480])
        .unwrap();
    element
        .c1
        .c2
        .c0
        .into_repr()
        .write_le(&mut buffer[480..528])
        .unwrap();
    element
        .c1
        .c2
        .c1
        .into_repr()
        .write_le(&mut buffer[528..LENGTH_FQ12_BYTES])
        .unwrap();
}
