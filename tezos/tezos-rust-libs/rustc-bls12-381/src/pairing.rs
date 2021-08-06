use super::reader::{read_fq12, read_uncompressed_g1, read_uncompressed_g2};
use super::writer::write_fq12;

use super::{LENGTH_FQ12_BYTES, LENGTH_UNCOMPRESSED_G1_BYTES, LENGTH_UNCOMPRESSED_G2_BYTES};
use group::EncodedPoint;
use pairing::bls12_381;
use pairing::Engine;
use pairing::PairingCurveAffine;

#[cfg(not(feature = "wasm"))]
use libc::c_uchar;

#[cfg(feature = "wasm")]
#[allow(non_camel_case_types)]
#[cfg(feature = "wasm")]
type c_uchar = u8;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_pairing_miller_loop_simple(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    g1: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g2: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
) {
    let g1 = unsafe { &*g1 };
    let g2 = unsafe { &*g2 };
    let g1 = read_uncompressed_g1(g1)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2 = read_uncompressed_g2(g2)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let result_miller_loop = bls12_381::Bls12::miller_loop([(&g1, &g2)].iter());
    write_fq12(buffer, result_miller_loop)
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_pairing_miller_loop_2(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    g1_1: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_2: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g2_1: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_2: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
) {
    let g1_1 = unsafe { &*g1_1 };
    let g1_2 = unsafe { &*g1_2 };
    let g2_1 = unsafe { &*g2_1 };
    let g2_2 = unsafe { &*g2_2 };
    let g1_1 = read_uncompressed_g1(g1_1)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_2 = read_uncompressed_g1(g1_2)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_1 = read_uncompressed_g2(g2_1)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_2 = read_uncompressed_g2(g2_2)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let result_miller_loop = bls12_381::Bls12::miller_loop([(&g1_1, &g2_1), (&g1_2, &g2_2)].iter());
    write_fq12(buffer, result_miller_loop)
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_pairing_miller_loop_3(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    g1_1: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_2: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_3: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g2_1: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_2: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_3: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
) {
    let g1_1 = unsafe { &*g1_1 };
    let g1_2 = unsafe { &*g1_2 };
    let g1_3 = unsafe { &*g1_3 };
    let g2_1 = unsafe { &*g2_1 };
    let g2_2 = unsafe { &*g2_2 };
    let g2_3 = unsafe { &*g2_3 };
    let g1_1 = read_uncompressed_g1(g1_1)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_2 = read_uncompressed_g1(g1_2)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_3 = read_uncompressed_g1(g1_3)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_1 = read_uncompressed_g2(g2_1)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_2 = read_uncompressed_g2(g2_2)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_3 = read_uncompressed_g2(g2_3)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let result_miller_loop =
        bls12_381::Bls12::miller_loop([(&g1_1, &g2_1), (&g1_2, &g2_2), (&g1_3, &g2_3)].iter());
    write_fq12(buffer, result_miller_loop)
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_pairing_miller_loop_4(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    g1_1: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_2: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_3: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_4: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g2_1: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_2: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_3: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_4: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
) {
    let g1_1 = unsafe { &*g1_1 };
    let g1_2 = unsafe { &*g1_2 };
    let g1_3 = unsafe { &*g1_3 };
    let g1_4 = unsafe { &*g1_4 };
    let g2_1 = unsafe { &*g2_1 };
    let g2_2 = unsafe { &*g2_2 };
    let g2_3 = unsafe { &*g2_3 };
    let g2_4 = unsafe { &*g2_4 };
    let g1_1 = read_uncompressed_g1(g1_1)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_2 = read_uncompressed_g1(g1_2)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_3 = read_uncompressed_g1(g1_3)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_4 = read_uncompressed_g1(g1_4)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_1 = read_uncompressed_g2(g2_1)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_2 = read_uncompressed_g2(g2_2)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_3 = read_uncompressed_g2(g2_3)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_4 = read_uncompressed_g2(g2_4)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let result_miller_loop = bls12_381::Bls12::miller_loop(
        [
            (&g1_1, &g2_1),
            (&g1_2, &g2_2),
            (&g1_3, &g2_3),
            (&g1_4, &g2_4),
        ]
        .iter(),
    );
    write_fq12(buffer, result_miller_loop)
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_pairing_miller_loop_5(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    g1_1: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_2: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_3: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_4: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_5: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g2_1: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_2: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_3: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_4: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_5: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
) {
    let g1_1 = unsafe { &*g1_1 };
    let g1_2 = unsafe { &*g1_2 };
    let g1_3 = unsafe { &*g1_3 };
    let g1_4 = unsafe { &*g1_4 };
    let g1_5 = unsafe { &*g1_5 };
    let g2_1 = unsafe { &*g2_1 };
    let g2_2 = unsafe { &*g2_2 };
    let g2_3 = unsafe { &*g2_3 };
    let g2_4 = unsafe { &*g2_4 };
    let g2_5 = unsafe { &*g2_5 };
    let g1_1 = read_uncompressed_g1(g1_1)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_2 = read_uncompressed_g1(g1_2)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_3 = read_uncompressed_g1(g1_3)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_4 = read_uncompressed_g1(g1_4)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_5 = read_uncompressed_g1(g1_5)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_1 = read_uncompressed_g2(g2_1)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_2 = read_uncompressed_g2(g2_2)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_3 = read_uncompressed_g2(g2_3)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_4 = read_uncompressed_g2(g2_4)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_5 = read_uncompressed_g2(g2_5)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let result_miller_loop = bls12_381::Bls12::miller_loop(
        [
            (&g1_1, &g2_1),
            (&g1_2, &g2_2),
            (&g1_3, &g2_3),
            (&g1_4, &g2_4),
            (&g1_5, &g2_5),
        ]
        .iter(),
    );
    write_fq12(buffer, result_miller_loop)
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_pairing_miller_loop_6(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    g1_1: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_2: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_3: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_4: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_5: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g1_6: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g2_1: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_2: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_3: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_4: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_5: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
    g2_6: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
) {
    let g1_1 = unsafe { &*g1_1 };
    let g1_2 = unsafe { &*g1_2 };
    let g1_3 = unsafe { &*g1_3 };
    let g1_4 = unsafe { &*g1_4 };
    let g1_5 = unsafe { &*g1_5 };
    let g1_6 = unsafe { &*g1_6 };
    let g2_1 = unsafe { &*g2_1 };
    let g2_2 = unsafe { &*g2_2 };
    let g2_3 = unsafe { &*g2_3 };
    let g2_4 = unsafe { &*g2_4 };
    let g2_5 = unsafe { &*g2_5 };
    let g2_6 = unsafe { &*g2_6 };
    let g1_1 = read_uncompressed_g1(g1_1)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_2 = read_uncompressed_g1(g1_2)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_3 = read_uncompressed_g1(g1_3)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_4 = read_uncompressed_g1(g1_4)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_5 = read_uncompressed_g1(g1_5)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g1_6 = read_uncompressed_g1(g1_6)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_1 = read_uncompressed_g2(g2_1)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_2 = read_uncompressed_g2(g2_2)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_3 = read_uncompressed_g2(g2_3)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_4 = read_uncompressed_g2(g2_4)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_5 = read_uncompressed_g2(g2_5)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let g2_6 = read_uncompressed_g2(g2_6)
        .into_affine_unchecked()
        .unwrap()
        .prepare();
    let result_miller_loop = bls12_381::Bls12::miller_loop(
        [
            (&g1_1, &g2_1),
            (&g1_2, &g2_2),
            (&g1_3, &g2_3),
            (&g1_4, &g2_4),
            (&g1_5, &g2_5),
            (&g1_6, &g2_6),
        ]
        .iter(),
    );
    write_fq12(buffer, result_miller_loop)
}

#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_pairing(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    g1: *const [c_uchar; LENGTH_UNCOMPRESSED_G1_BYTES],
    g2: *const [c_uchar; LENGTH_UNCOMPRESSED_G2_BYTES],
) {
    let g1 = unsafe { &*g1 };
    let g2 = unsafe { &*g2 };
    let g1 = read_uncompressed_g1(g1).into_affine_unchecked().unwrap();
    let g2 = read_uncompressed_g2(g2).into_affine_unchecked().unwrap();
    let result_pairing = bls12_381::Bls12::pairing(g1, g2);
    write_fq12(buffer, result_pairing)
}

// Do not check if x is null, hence unsafe
#[cfg_attr(not(feature = "wasm"), no_mangle)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub extern "C" fn rustc_bls12_381_unsafe_pairing_final_exponentiation(
    buffer: *mut [c_uchar; LENGTH_FQ12_BYTES],
    x: *const [c_uchar; LENGTH_FQ12_BYTES],
) {
    let x = read_fq12(unsafe { &*x }).unwrap();
    let x = bls12_381::Bls12::final_exponentiation(&x).unwrap();
    write_fq12(buffer, x);
}
