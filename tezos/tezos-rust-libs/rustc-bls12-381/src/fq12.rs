use ff::Field;
use pairing::bls12_381;
use pairing::bls12_381::Fq12;

use super::reader::read_fq12;
use super::writer::write_fq12;

use super::{LENGTH_FQ12_BYTES, LENGTH_FQ12_U64};
use rand::rngs::OsRng;
use std::convert::TryInto;

#[cfg(not(feature = "wasm"))]
use libc::c_uchar;

#[cfg(feature = "wasm")]
#[allow(non_camel_case_types)]
#[cfg(feature = "wasm")]
type c_uchar = u8;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

// Check that a byte array represents a valid point.
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fq12_check_bytes(
    element: *const [c_uchar; LENGTH_FQ12_BYTES],
) -> bool {
    read_fq12(unsafe { &*element }).is_ok()
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fq12_is_zero(g: *const [c_uchar; LENGTH_FQ12_BYTES]) -> bool {
    let g = read_fq12(unsafe { &*g }).unwrap();
    g.is_zero()
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fq12_is_one(g: *const [c_uchar; LENGTH_FQ12_BYTES]) -> bool {
    let g = read_fq12(unsafe { &*g }).unwrap();
    g == Fq12::one()
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fq12_random(buffer: *mut [c_uchar; LENGTH_FQ12_BYTES]) {
    let random = bls12_381::Fq12::random(&mut OsRng);
    write_fq12(buffer, random);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fq12_one(buffer: *mut [c_uchar; LENGTH_FQ12_BYTES]) {
    let one = bls12_381::Fq12::one();
    write_fq12(buffer, one);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fq12_zero(buffer: *mut [c_uchar; LENGTH_FQ12_BYTES]) {
    let zero = bls12_381::Fq12::zero();
    write_fq12(buffer, zero);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fq12_add(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    x: *const [c_uchar; LENGTH_FQ12_BYTES],
    y: *const [c_uchar; LENGTH_FQ12_BYTES],
) {
    let mut x = read_fq12(unsafe { &*x }).unwrap();
    let y = read_fq12(unsafe { &*y }).unwrap();
    x.add_assign(&y);
    write_fq12(buffer, x);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fq12_mul(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    x: *const [c_uchar; LENGTH_FQ12_BYTES],
    y: *const [c_uchar; LENGTH_FQ12_BYTES],
) {
    let mut x = read_fq12(unsafe { &*x }).unwrap();
    let y = read_fq12(unsafe { &*y }).unwrap();
    x.mul_assign(&y);
    write_fq12(buffer, x);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fq12_eq(
    x: *const [c_uchar; LENGTH_FQ12_BYTES],
    y: *const [c_uchar; LENGTH_FQ12_BYTES],
) -> bool {
    let x = read_fq12(unsafe { &*x }).unwrap();
    let y = read_fq12(unsafe { &*y }).unwrap();
    x == y
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fq12_unsafe_inverse(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    x: *const [c_uchar; LENGTH_FQ12_BYTES],
) {
    let x = read_fq12(unsafe { &*x }).unwrap();
    let x = x.inverse().unwrap();
    write_fq12(buffer, x);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fq12_negate(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    x: *const [c_uchar; LENGTH_FQ12_BYTES],
) {
    let mut x = read_fq12(unsafe { &*x }).unwrap();
    x.negate();
    write_fq12(buffer, x);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fq12_square(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    x: *const [c_uchar; LENGTH_FQ12_BYTES],
) {
    let mut x = read_fq12(unsafe { &*x }).unwrap();
    x.square();
    write_fq12(buffer, x)
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fq12_double(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    x: *const [c_uchar; LENGTH_FQ12_BYTES],
) {
    let mut x = read_fq12(unsafe { &*x }).unwrap();
    x.double();
    write_fq12(buffer, x);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fq12_pow(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    x: *const [c_uchar; LENGTH_FQ12_BYTES],
    n: *const [c_uchar; LENGTH_FQ12_BYTES],
) {
    let x = read_fq12(unsafe { &*x }).unwrap();
    let n = unsafe { *n };
    let mut n_u64: [u64; LENGTH_FQ12_U64] = [0; LENGTH_FQ12_U64];
    for i in 0..LENGTH_FQ12_U64 {
        n_u64[i] = u64::from_le_bytes(n[i * 8..(i + 1) * 8].try_into().unwrap())
    }
    let res = x.pow(n_u64.as_ref());
    write_fq12(buffer, res)
}
