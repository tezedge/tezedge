use ff::{PrimeField, PrimeFieldRepr};
use group::{CurveAffine, CurveProjective, EncodedPoint};
use pairing::bls12_381;

use super::reader::{read_compressed_g1, read_fq, read_fr, read_uncompressed_g1};
use super::writer::{write_compressed_g1, write_uncompressed_g1};

use super::{
    LENGTH_COMPRESSED_G1_BYTES, LENGTH_FQ_BYTES, LENGTH_FR_BYTES, LENGTH_UNCOMPRESSED_G1_BYTES,
};

use rand::rngs::OsRng;

#[cfg(not(feature = "wasm"))]
use libc::c_uchar;

#[cfg(feature = "wasm")]
#[allow(non_camel_case_types)]
#[cfg(feature = "wasm")]
type c_uchar = u8;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

// Check that a byte array represents a valid uncompressed point.
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_uncompressed_check_bytes(
    uncompressed: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
) -> bool {
    read_uncompressed_g1(unsafe { &*uncompressed })
        .into_affine()
        .is_ok()
}

// Check that a byte array represents a valid compressed point.
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_compressed_check_bytes(
    compressed: *const [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
) -> bool {
    read_compressed_g1(unsafe { &*compressed })
        .into_affine()
        .is_ok()
}

// Get the compressed version of a point from the uncompressed version

pub(crate) fn g1_compressed_of_uncompressed(
    g1: bls12_381::G1Uncompressed,
) -> bls12_381::G1Compressed {
    let uncompressed = g1.into_affine_unchecked().unwrap();
    let compressed = bls12_381::G1Compressed::from_affine(uncompressed);
    compressed
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_compressed_of_uncompressed(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
    uncompressed: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
) {
    let uncompressed = unsafe { &*uncompressed };
    let uncompressed = read_uncompressed_g1(&uncompressed);
    let compressed = g1_compressed_of_uncompressed(uncompressed);
    write_compressed_g1(buffer, compressed)
}

pub(crate) fn g1_uncompressed_of_compressed(
    g: bls12_381::G1Compressed,
) -> bls12_381::G1Uncompressed {
    let compressed = g.into_affine_unchecked().unwrap();
    let uncompressed = bls12_381::G1Uncompressed::from_affine(compressed);
    uncompressed
}
// Get the uncompressed version of a point from the compressed version
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_uncompressed_of_compressed(
    buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    compressed: *const [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
) {
    let compressed = unsafe { &*compressed };
    let compressed = read_compressed_g1(&compressed);
    let uncompressed = g1_uncompressed_of_compressed(compressed);
    write_uncompressed_g1(buffer, uncompressed)
}

pub(crate) fn g1_uncompressed_generate_zero() -> bls12_381::G1Uncompressed {
    let zero = bls12_381::G1Affine::zero();
    let uncompressed_form = bls12_381::G1Uncompressed::from_affine(zero);
    uncompressed_form
}

// Get the zero of the curve
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_zero(buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES]) {
    let uncompressed_form = g1_uncompressed_generate_zero();
    write_uncompressed_g1(buffer, uncompressed_form);
}

pub(crate) fn g1_uncompressed_generate_one() -> bls12_381::G1Uncompressed {
    let one = bls12_381::G1Affine::one();
    let uncompressed_form = bls12_381::G1Uncompressed::from_affine(one);
    uncompressed_form
}
// Get a fixed generator of the curve
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_one(buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES]) {
    let uncompressed_form = g1_uncompressed_generate_one();
    write_uncompressed_g1(buffer, uncompressed_form);
}

// Get a random point on the curve
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_random(buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES]) {
    let random_g1 = bls12_381::G1::random(&mut OsRng);
    let uncompressed_form = bls12_381::G1Uncompressed::from_affine(random_g1.into_affine());
    write_uncompressed_g1(buffer, uncompressed_form);
}

pub(crate) fn g1_uncompressed_add(
    g1: bls12_381::G1Uncompressed,
    g2: bls12_381::G1Uncompressed,
) -> bls12_381::G1Uncompressed {
    let mut g1 = g1.into_affine_unchecked().unwrap().into_projective();
    let g2 = g2.into_affine_unchecked().unwrap().into_projective();
    g1.add_assign(&g2);
    bls12_381::G1Uncompressed::from_affine(g1.into_affine())
}
// Add two points of the curve.
// !! The two points must be valid. Undefined behaviors otherwise.
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_add(
    buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g2: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
) {
    let g1 = read_uncompressed_g1(unsafe { &*g1 });
    let g2 = read_uncompressed_g1(unsafe { &*g2 });
    let result = g1_uncompressed_add(g1, g2);
    write_uncompressed_g1(buffer, result);
}

// Compute the opposite of the point.
// !! The point must be valid. Undefined behaviors otherwise.
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_negate(
    buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
) {
    let g = unsafe { &*g };
    let g = read_uncompressed_g1(&g);
    let mut g = g.into_affine_unchecked().unwrap();
    g.negate();
    let g = bls12_381::G1Uncompressed::from_affine(g);
    write_uncompressed_g1(buffer, g);
}

// Check if two points are algebraically equals
// !! The points must be valid. Undefined behaviors otherwise.
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_eq(
    g1: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g2: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
) -> bool {
    let g1 = unsafe { &*g1 };
    let g2 = unsafe { &*g2 };
    let g1 = read_uncompressed_g1(&g1).into_affine_unchecked().unwrap();
    let g2 = read_uncompressed_g1(&g2).into_affine_unchecked().unwrap();
    g1 == g2
}

// Check if the given point is the zero of the curve
// !! The point must be valid. Undefined behaviors otherwise.
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_is_zero(
    g: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
) -> bool {
    let g = unsafe { &*g };
    let g = read_uncompressed_g1(&g).into_affine_unchecked().unwrap();
    g.is_zero()
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_double(
    buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
) {
    let g = read_uncompressed_g1(unsafe { &*g });
    // FIXME: A bit costy to go back in repr.
    let mut g = g.into_affine_unchecked().unwrap().into_projective();
    g.double();
    let result = bls12_381::G1Uncompressed::from_affine(g.into_affine());
    write_uncompressed_g1(buffer, result);
}

// Multiply a point by a scalar (an Fr element)
// !! The point must be valid. Undefined behaviors otherwise.
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_mul(
    buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    a: *const [c_uchar; LENGTH_FR_BYTES],
) {
    let g = read_uncompressed_g1(unsafe { &*g });
    // FIXME: A bit costy to go back in repr.
    let mut g = g.into_affine_unchecked().unwrap().into_projective();
    let a = read_fr(unsafe { &*a }).unwrap().into_repr();
    g.mul_assign(a);
    let result = bls12_381::G1Uncompressed::from_affine(g.into_affine());
    write_uncompressed_g1(buffer, result);
}

// --------------- G1 Compressed ---------------------
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_compressed_eq(
    g1: *const [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
    g2: *const [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
) -> bool {
    let g1 = unsafe { &*g1 };
    let g2 = unsafe { &*g2 };
    let g1 = read_compressed_g1(&g1).into_affine_unchecked().unwrap();
    let g2 = read_compressed_g1(&g2).into_affine_unchecked().unwrap();
    g1 == g2
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_compressed_negate(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
    g: *const [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
) {
    let g = unsafe { &*g };
    let g = read_compressed_g1(&g);
    let mut g = g.into_affine_unchecked().unwrap();
    g.negate();
    let g = bls12_381::G1Compressed::from_affine(g);
    write_compressed_g1(buffer, g);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_compressed_add(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
    g1: *const [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
    g2: *const [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
) {
    let g1 = unsafe { &*g1 };
    let g2 = unsafe { &*g2 };
    let mut g1 = read_compressed_g1(&g1)
        .into_affine_unchecked()
        .unwrap()
        .into_projective();
    let g2 = read_compressed_g1(&g2)
        .into_affine_unchecked()
        .unwrap()
        .into_projective();
    g1.add_assign(&g2);
    let result = bls12_381::G1Compressed::from_affine(g1.into_affine());
    write_compressed_g1(buffer, result);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_compressed_is_zero(
    g: *const [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
) -> bool {
    let g = unsafe { &*g };
    let g = read_compressed_g1(&g).into_affine_unchecked().unwrap();
    g.is_zero()
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_compressed_zero(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
) {
    let zero = bls12_381::G1Affine::zero();
    let compressed_form = bls12_381::G1Compressed::from_affine(zero);
    write_compressed_g1(buffer, compressed_form);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_compressed_one(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
) {
    let one = bls12_381::G1Affine::one();
    let compressed_form = bls12_381::G1Compressed::from_affine(one);
    write_compressed_g1(buffer, compressed_form);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_compressed_random(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
) {
    let random_g1 = bls12_381::G1::random(&mut OsRng);
    let compressed_form = bls12_381::G1Compressed::from_affine(random_g1.into_affine());
    write_compressed_g1(buffer, compressed_form);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_compressed_double(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
    g: *const [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
) {
    let g = read_compressed_g1(unsafe { &*g });
    // FIXME: A bit costy to go back in repr.
    let mut g = g.into_affine_unchecked().unwrap().into_projective();
    g.double();
    let result = bls12_381::G1Compressed::from_affine(g.into_affine());
    write_compressed_g1(buffer, result);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_compressed_mul(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
    g: *const [c_uchar; LENGTH_COMPRESSED_G1_BYTES],
    a: *const [c_uchar; LENGTH_FR_BYTES],
) {
    let g = read_compressed_g1(unsafe { &*g });
    // FIXME: A bit costy to go back in repr.
    let mut g = g.into_affine_unchecked().unwrap().into_projective();
    let a = read_fr(unsafe { &*a }).unwrap().into_repr();
    g.mul_assign(a);
    let result = bls12_381::G1Compressed::from_affine(g.into_affine());
    write_compressed_g1(buffer, result);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g1_build_from_components(
    buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    x: *const [c_uchar; LENGTH_FQ_BYTES],
    y: *const [c_uchar; LENGTH_FQ_BYTES],
) -> bool {
    let x = read_fq(unsafe { &*x });
    let y = read_fq(unsafe { &*y });
    match (x, y) {
        (Ok(x), Ok(y)) => {
            let mut buffer_g1_point: [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES] =
                [0; LENGTH_UNCOMPRESSED_G1_BYTES];
            x.into_repr()
                .write_be(&mut buffer_g1_point[..LENGTH_FQ_BYTES])
                .unwrap();
            y.into_repr()
                .write_be(&mut buffer_g1_point[LENGTH_FQ_BYTES..])
                .unwrap();
            let g1_point = read_uncompressed_g1(&buffer_g1_point);
            if g1_point.into_affine().is_ok() {
                write_uncompressed_g1(buffer, g1_point);
                true
            } else {
                false
            }
        }
        _ => false,
    }
}
