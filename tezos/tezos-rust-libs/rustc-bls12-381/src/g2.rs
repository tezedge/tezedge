use ff::{PrimeField, PrimeFieldRepr};
use group::{CurveAffine, CurveProjective, EncodedPoint};
use pairing::bls12_381;

use super::reader::{read_compressed_g2, read_fq, read_fr, read_uncompressed_g2};
use super::writer::{write_compressed_g2, write_uncompressed_g2};

use super::{
    LENGTH_COMPRESSED_G2_BYTES, LENGTH_FQ_BYTES, LENGTH_FR_BYTES, LENGTH_UNCOMPRESSED_G2_BYTES,
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
pub extern "C" fn rustc_bls12_381_g2_uncompressed_check_bytes(
    uncompressed: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
) -> bool {
    read_uncompressed_g2(unsafe { &*uncompressed })
        .into_affine()
        .is_ok()
}

// Check that a byte array represents a valid compressed point.
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_compressed_check_bytes(
    compressed: *const [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
) -> bool {
    read_compressed_g2(unsafe { &*compressed })
        .into_affine()
        .is_ok()
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_compressed_of_uncompressed(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
    uncompressed: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
) {
    let uncompressed = unsafe { &*uncompressed };
    let uncompressed = read_uncompressed_g2(&uncompressed);
    let uncompressed = uncompressed.into_affine_unchecked().unwrap();
    let compressed = bls12_381::G2Compressed::from_affine(uncompressed);
    write_compressed_g2(buffer, compressed)
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_uncompressed_of_compressed(
    buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    compressed: *const [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
) {
    let compressed = unsafe { &*compressed };
    let compressed = read_compressed_g2(&compressed);
    let compressed = compressed.into_affine_unchecked().unwrap();
    let uncompressed = bls12_381::G2Uncompressed::from_affine(compressed);
    write_uncompressed_g2(buffer, uncompressed)
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_zero(buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES]) {
    let zero = bls12_381::G2Affine::zero();
    let uncompressed_form = bls12_381::G2Uncompressed::from_affine(zero);
    write_uncompressed_g2(buffer, uncompressed_form);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_one(buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES]) {
    let one = bls12_381::G2Affine::one();
    let uncompressed_form = bls12_381::G2Uncompressed::from_affine(one);
    write_uncompressed_g2(buffer, uncompressed_form);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_random(buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES]) {
    let random_g1 = bls12_381::G2::random(&mut OsRng);
    let uncompressed_form = bls12_381::G2Uncompressed::from_affine(random_g1.into_affine());
    write_uncompressed_g2(buffer, uncompressed_form);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_add(
    buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g1: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
) {
    let g1 = unsafe { &*g1 };
    let g2 = unsafe { &*g2 };
    let mut g1 = read_uncompressed_g2(&g1)
        .into_affine_unchecked()
        .unwrap()
        .into_projective();
    let g2 = read_uncompressed_g2(&g2)
        .into_affine_unchecked()
        .unwrap()
        .into_projective();
    g1.add_assign(&g2);
    let sum = bls12_381::G2Uncompressed::from_affine(g1.into_affine());
    write_uncompressed_g2(buffer, sum);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_negate(
    buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
) {
    // FIXME/IMPROVEME: It should be possible to not use the projective representation. it has a cost.
    let g = unsafe { &*g };
    let mut g = read_uncompressed_g2(&g).into_affine_unchecked().unwrap();
    g.negate();
    let opposite = bls12_381::G2Uncompressed::from_affine(g);
    write_uncompressed_g2(buffer, opposite);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_eq(
    g1: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
) -> bool {
    let g1 = unsafe { &*g1 };
    let g2 = unsafe { &*g2 };
    let g1 = read_uncompressed_g2(&g1).into_affine_unchecked().unwrap();
    let g2 = read_uncompressed_g2(&g2).into_affine_unchecked().unwrap();
    g1 == g2
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_is_zero(
    g: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
) -> bool {
    let g = unsafe { &*g };
    let g = read_uncompressed_g2(&g).into_affine_unchecked().unwrap();
    g.is_zero()
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_double(
    buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
) {
    let g = read_uncompressed_g2(unsafe { &*g });
    // FIXME: A bit costy to go back in repr.
    let mut g = g.into_affine_unchecked().unwrap().into_projective();
    g.double();
    let result = bls12_381::G2Uncompressed::from_affine(g.into_affine());
    write_uncompressed_g2(buffer, result);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_mul(
    buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    a: *const [c_uchar; LENGTH_FR_BYTES],
) {
    let g = read_uncompressed_g2(unsafe { &*g });
    // FIXME: A bit costy to go back in repr.
    let a = read_fr(unsafe { &*a }).unwrap().into_repr();
    let mut result = g.into_affine_unchecked().unwrap().into_projective();
    result.mul_assign(a);
    let result = bls12_381::G2Uncompressed::from_affine(result.into_affine());
    write_uncompressed_g2(buffer, result);
}

// --------------- G2 Compressed ---------------------
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_compressed_zero(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
) {
    let zero = bls12_381::G2Affine::zero();
    let compressed_form = bls12_381::G2Compressed::from_affine(zero);
    write_compressed_g2(buffer, compressed_form);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_compressed_one(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
) {
    let one = bls12_381::G2Affine::one();
    let compressed_form = bls12_381::G2Compressed::from_affine(one);
    write_compressed_g2(buffer, compressed_form);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_compressed_random(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
) {
    let random_g1 = bls12_381::G2::random(&mut OsRng);
    let compressed_form = bls12_381::G2Compressed::from_affine(random_g1.into_affine());
    write_compressed_g2(buffer, compressed_form);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_compressed_add(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
    g1: *const [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
    g2: *const [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
) {
    let g1 = unsafe { &*g1 };
    let g2 = unsafe { &*g2 };
    let mut g1 = read_compressed_g2(&g1)
        .into_affine_unchecked()
        .unwrap()
        .into_projective();
    let g2 = read_compressed_g2(&g2)
        .into_affine_unchecked()
        .unwrap()
        .into_projective();
    g1.add_assign(&g2);
    let sum = bls12_381::G2Compressed::from_affine(g1.into_affine());
    write_compressed_g2(buffer, sum);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_compressed_negate(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
    g: *const [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
) {
    // FIXME/IMPROVEME: It should be possible to not use the projective representation. it has a cost.
    let g = unsafe { &*g };
    let mut g = read_compressed_g2(&g).into_affine_unchecked().unwrap();
    g.negate();
    let opposite = bls12_381::G2Compressed::from_affine(g);
    write_compressed_g2(buffer, opposite);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_compressed_eq(
    g1: *const [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
    g2: *const [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
) -> bool {
    let g1 = unsafe { &*g1 };
    let g2 = unsafe { &*g2 };
    let g1 = read_compressed_g2(&g1).into_affine_unchecked().unwrap();
    let g2 = read_compressed_g2(&g2).into_affine_unchecked().unwrap();
    g1 == g2
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_compressed_is_zero(
    g: *const [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
) -> bool {
    let g = unsafe { &*g };
    let g = read_compressed_g2(&g).into_affine_unchecked().unwrap();
    g.is_zero()
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_compressed_double(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
    g: *const [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
) {
    let g = read_compressed_g2(unsafe { &*g });
    let mut g = g.into_affine_unchecked().unwrap().into_projective();
    g.double();
    let result = bls12_381::G2Compressed::from_affine(g.into_affine());
    write_compressed_g2(buffer, result);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_compressed_mul(
    buffer: *mut [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
    g: *const [c_uchar; LENGTH_COMPRESSED_G2_BYTES],
    a: *const [c_uchar; LENGTH_FR_BYTES],
) {
    let g = read_compressed_g2(unsafe { &*g });
    // FIXME: A bit costy to go back in repr.
    let a = read_fr(unsafe { &*a }).unwrap().into_repr();
    let mut result = g.into_affine_unchecked().unwrap().into_projective();
    result.mul_assign(a);
    let result = bls12_381::G2Compressed::from_affine(result.into_affine());
    write_compressed_g2(buffer, result);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_g2_build_from_components(
    buffer: *mut [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    x_c0: *const [c_uchar; LENGTH_FQ_BYTES],
    x_c1: *const [c_uchar; LENGTH_FQ_BYTES],
    y_c0: *const [c_uchar; LENGTH_FQ_BYTES],
    y_c1: *const [c_uchar; LENGTH_FQ_BYTES],
) -> bool {
    let x_c0 = read_fq(unsafe { &*x_c0 });
    let x_c1 = read_fq(unsafe { &*x_c1 });
    let y_c0 = read_fq(unsafe { &*y_c0 });
    let y_c1 = read_fq(unsafe { &*y_c1 });
    match (x_c0, x_c1, y_c0, y_c1) {
        (Ok(x_c0), Ok(x_c1), Ok(y_c0), Ok(y_c1)) => {
            let mut buffer_g2_point: [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES] =
                [0; LENGTH_UNCOMPRESSED_G2_BYTES];
            x_c1.into_repr()
                .write_be(&mut buffer_g2_point[..LENGTH_FQ_BYTES])
                .unwrap();
            x_c0.into_repr()
                .write_be(&mut buffer_g2_point[LENGTH_FQ_BYTES..LENGTH_FQ_BYTES * 2])
                .unwrap();
            y_c1.into_repr()
                .write_be(&mut buffer_g2_point[LENGTH_FQ_BYTES * 2..LENGTH_FQ_BYTES * 3])
                .unwrap();
            y_c0.into_repr()
                .write_be(&mut buffer_g2_point[LENGTH_FQ_BYTES * 3..])
                .unwrap();
            let g2_point = read_uncompressed_g2(&buffer_g2_point);
            if g2_point.into_affine().is_ok() {
                write_uncompressed_g2(buffer, g2_point);
                true
            } else {
                false
            }
        }
        _ => false,
    }
}
