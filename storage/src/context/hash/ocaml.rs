// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Port of OCaml's string hashing function

use std::{convert::TryInto, num::Wrapping};

// MIX macro:
//  https://github.com/ocaml/ocaml/blob/a5f63ba65fb8df7b2fa52076a2763b74078e463e/runtime/hash.c#L33
#[inline]
fn mix(mut h: Wrapping<u32>, mut w: Wrapping<u32>) -> Wrapping<u32> {
    w *= Wrapping(0xcc9e2d51);
    w = w << 15 | w >> 17;
    w *= Wrapping(0x1b873593);
    h ^= w;
    h = h << 13 | h >> 19;
    h = h * Wrapping(5) + Wrapping(0xe6546b64);
    h
}

// caml_hash_mix_string function:
//   https://github.com/ocaml/ocaml/blob/a5f63ba65fb8df7b2fa52076a2763b74078e463e/runtime/hash.c#L145
pub fn ocaml_hash_string(seed: u32, s: &[u8]) -> u32 {
    let len = s.len() as u32;
    let mut h = Wrapping(seed);
    let mut i = 0;

    // Mix by 32-bit blocks (little-endian)
    while i + 4 <= len {
        let pos = i as usize;
        let next_u32_bytes: [u8; 4] = s[pos..pos + 4].try_into().unwrap();
        let w = Wrapping(u32::from_le_bytes(next_u32_bytes));

        h = mix(h, w);
        i += 4;
    }

    // At this point there might be up to 3 bytes left to read.
    // Bytes that are out of range should be set to \000.
    let pos = i as usize;
    h = match len & 3 {
        3 => mix(
            h,
            Wrapping((s[pos + 2] as u32) << 16 | (s[pos + 1] as u32) << 8 | (s[pos] as u32)),
        ),
        2 => mix(h, Wrapping((s[pos + 1] as u32) << 8 | (s[pos] as u32))),
        1 => mix(h, Wrapping(s[pos] as u32)),
        _ => h, // len & 3 == 0, no extra bytes, do nothing
    };
    h ^= Wrapping(len);

    // Final mix:
    //   https://github.com/ocaml/ocaml/blob/a5f63ba65fb8df7b2fa52076a2763b74078e463e/runtime/hash.c#L41-L46
    h ^= h >> 16;
    h *= Wrapping(0x85ebca6b);
    h ^= h >> 13;
    h *= Wrapping(0xc2b2ae35);
    h ^= h >> 16;

    // Fold result to the range [0, 2^30-1] so that it is a nonnegative
    // OCaml integer both on 32 and 64-bit platforms.
    h.0 & 0x3FFFFFFF
}

#[cfg(test)]
mod tests {
    use super::ocaml_hash_string;

    #[test]
    fn test_ocaml_hash_string() {
        // Left-side values obtained with OCaml: Hashtbl.hash "the-string"
        assert_eq!(0, ocaml_hash_string(0, b""));
        assert_eq!(721651713, ocaml_hash_string(0, b"a"));
        assert_eq!(856662637, ocaml_hash_string(0, b"ab"));
        assert_eq!(767105082, ocaml_hash_string(0, b"abc"));
        assert_eq!(65890154, ocaml_hash_string(0, b"abcd"));
        assert_eq!(335633756, ocaml_hash_string(0, b"abcde"));
        assert_eq!(926323203, ocaml_hash_string(0, b"abcdef"));
    }
}
