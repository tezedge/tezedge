use ff::Field;
use pairing::bls12_381;
use pairing::bls12_381::Fr;

use super::reader::read_fr;
use super::writer::write_fr;
use rand::rngs::OsRng;

use super::{LENGTH_FR_BYTES, LENGTH_FR_U64};
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
pub extern "C" fn rustc_bls12_381_fr_check_bytes(
    element: *const [c_uchar; LENGTH_FR_BYTES],
) -> bool {
    read_fr(unsafe { &*element }).is_ok()
}
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fr_is_zero(element: *const [c_uchar; LENGTH_FR_BYTES]) -> bool {
    let fr_elem = read_fr(unsafe { &*element });
    match fr_elem {
        Err(_) => false,
        Ok(fr_elem) => fr_elem.is_zero(),
    }
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fr_is_one(element: *const [c_uchar; LENGTH_FR_BYTES]) -> bool {
    let fr_elem = read_fr(unsafe { &*element });
    match fr_elem {
        Err(_) => false,
        Ok(fr_elem) => fr_elem == bls12_381::Fr::one(),
    }
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fr_zero(buffer: *mut [c_uchar; LENGTH_FR_BYTES]) {
    let zero = bls12_381::Fr::zero();
    write_fr(buffer, zero);
    // FIXME: Not sure the next line does work and/or may introduce bugs
    // assert!(rustc_bls12_381_fr_is_zero(buffer), true);
    ()
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fr_one(buffer: *mut [c_uchar; LENGTH_FR_BYTES]) {
    let one = bls12_381::Fr::one();
    write_fr(buffer, one);
    // FIXME: Not sure the next line does work and/or may introduce bugs
    // assert!(rustc_bls12_381_fr_is_one(buffer), true);
    ()
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fr_random(buffer: *mut [c_uchar; LENGTH_FR_BYTES]) {
    let random_x = Fr::random(&mut OsRng);
    write_fr(buffer, random_x);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fr_add(
    buffer: *mut [c_uchar; LENGTH_FR_BYTES],
    x: *const [c_uchar; LENGTH_FR_BYTES],
    y: *const [c_uchar; LENGTH_FR_BYTES],
) {
    let mut x = read_fr(unsafe { &*x }).unwrap();
    let y = read_fr(unsafe { &*y }).unwrap();
    x.add_assign(&y);
    write_fr(buffer, x);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fr_mul(
    buffer: *mut [c_uchar; LENGTH_FR_BYTES],
    x: *const [c_uchar; LENGTH_FR_BYTES],
    y: *const [c_uchar; LENGTH_FR_BYTES],
) {
    let mut x = read_fr(unsafe { &*x }).unwrap();
    let y = read_fr(unsafe { &*y }).unwrap();
    x.mul_assign(&y);
    write_fr(buffer, x);
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fr_eq(
    x: *const [c_uchar; LENGTH_FR_BYTES],
    y: *const [c_uchar; LENGTH_FR_BYTES],
) -> bool {
    // FIXME/IMPROVEME: Is it the correct way of checking the equality? Depends
    // on the library implementation of ==.
    let x = read_fr(unsafe { &*x }).unwrap();
    let y = read_fr(unsafe { &*y }).unwrap();
    x == y
}

/* BE CAREFULE: DO NOT CHECK IF INVERSIBLE!!! */
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fr_unsafe_inverse(
    buffer: *mut [c_uchar; LENGTH_FR_BYTES],
    x: *const [c_uchar; LENGTH_FR_BYTES],
) {
    let x = read_fr(unsafe { &*x }).unwrap();
    let x = x.inverse().unwrap();
    write_fr(buffer, x)
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fr_negate(
    buffer: *mut [c_uchar; LENGTH_FR_BYTES],
    x: *const [c_uchar; LENGTH_FR_BYTES],
) {
    let mut x = read_fr(unsafe { &*x }).unwrap();
    x.negate();
    write_fr(buffer, x)
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fr_square(
    buffer: *mut [c_uchar; LENGTH_FR_BYTES],
    x: *const [c_uchar; LENGTH_FR_BYTES],
) {
    let mut x = read_fr(unsafe { &*x }).unwrap();
    x.square();
    write_fr(buffer, x)
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fr_double(
    buffer: *mut [c_uchar; LENGTH_FR_BYTES],
    x: *const [c_uchar; LENGTH_FR_BYTES],
) {
    let mut x = read_fr(unsafe { &*x }).unwrap();
    x.double();
    write_fr(buffer, x)
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_fr_pow(
    buffer: *mut [c_uchar; LENGTH_FR_BYTES],
    x: *const [c_uchar; LENGTH_FR_BYTES],
    n: *const [c_uchar; LENGTH_FR_BYTES],
) {
    let x = read_fr(unsafe { &*x }).unwrap();
    let n = unsafe { *n };
    let mut n_u64: [u64; LENGTH_FR_U64] = [0; LENGTH_FR_U64];
    for i in 0..LENGTH_FR_U64 {
        n_u64[i] = u64::from_le_bytes(n[i * 8..(i + 1) * 8].try_into().unwrap())
    }
    let res = x.pow(n_u64.as_ref());
    write_fr(buffer, res)
}

#[cfg(test)]
mod tests {
    use super::*;
    // FIXME: It does not work!!! Please write tests. At the moment, the
    // correctness is verified using the OCaml binding.
    #[test]
    fn test_write_followed_by_read() {
        let zero = bls12_381::Fr::zero();
        let mut buffer: [c_uchar; LENGTH_FR_BYTES] = [0; LENGTH_FR_BYTES];
        let read_ptr_buffer: *const [c_uchar; LENGTH_FR_BYTES] = &buffer;
        let write_ptr_buffer: *mut [c_uchar; LENGTH_FR_BYTES] = &mut buffer;

        write_fr(write_ptr_buffer, zero);
        let read_value = read_fr(unsafe { &*read_ptr_buffer });
        assert!(read_value.unwrap().is_zero(), true)
    }

    #[test]
    fn test_rustc_bls12_381_fr_is_zero() {
        let zero = bls12_381::Fr::zero();
        let mut buffer: [c_uchar; LENGTH_FR_BYTES] = [0; LENGTH_FR_BYTES];
        let write_ptr_buffer: *mut [c_uchar; LENGTH_FR_BYTES] = &mut buffer;
        let read_ptr_buffer: *const [c_uchar; LENGTH_FR_BYTES] = &buffer;
        write_fr(write_ptr_buffer, zero);
        assert!(rustc_bls12_381_fr_is_zero(read_ptr_buffer), true)
    }
}
