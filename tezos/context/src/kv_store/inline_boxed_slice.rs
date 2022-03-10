// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! A pointer to a slice of bytes
//!
//! - When the length of the bytes is less than 15 bytes, the bytes are inlined
//!   in the pointer.
//! - When the length of the bytes is between 15 and 114688, the bytes
//!   are in a Box<[u8; N]> with `N` being a predefined length: 16, 32, 48, ..
//!   Those length are taken from the sizes classes of `jemalloc`, allocating a
//!   number of bytes will use the size class just above, or equal to the number
//!   of bytes.
//!   For example, allocating 28 bytes with jemalloc will actually use 32 bytes.
//! - When the length of bytes is bigger, it's using a double Box:
//!   `Box<Box<[u8]>` so that the `InlinedBoxSlice` remains small (16 bytes)
//!   So far this length of bytes was never reached
//!
//! http://jemalloc.net/jemalloc.3.html
//!
use static_assertions::assert_eq_size;

macro_rules! super_box (
    (
        $( { $name:tt, $N:expr } ),*
    ) => (
        mod defaults {
            $(pub(super) const $name: [u8; $N] = [0; $N];)*
        }

        #[derive(Clone, Debug)]
        pub enum InlinedBoxedSlice {
            Inlined { bytes: [u8; 15] },
            $($name { length: u32, boxed: Box<[u8; $N]> },)*
            Huge { boxed: Box<Box<[u8]>> },
        }

        impl InlinedBoxedSlice {
            fn from_slice(slice: &[u8]) -> Self {
                let length = slice.len();

                match length {
                    x if x < 15 => {
                        let mut bytes = <[u8; 15]>::default();
                        bytes[0] = length as u8;
                        bytes[1..1 + length].copy_from_slice(slice);
                        Self::Inlined { bytes }
                    }
                    $(x if x <= $N => {
                        let mut boxed = Box::<[u8; $N]>::from(defaults::$name);
                        boxed[0..length].copy_from_slice(slice);
                        let length = length as u32;
                        Self::$name { length, boxed }
                    })*
                    _ => {
                        let boxed = Box::from(slice);
                        Self::Huge { boxed: Box::new(boxed) }
                    }
                }
            }

            fn as_slice(&self) -> &[u8] {
                match self {
                    InlinedBoxedSlice::Inlined { bytes } => {
                        let length = bytes[0] as usize;
                        &bytes[1..1 + length]
                    },
                    $(InlinedBoxedSlice::$name { length, boxed } => {
                        &boxed[0..*length as usize]
                    },)*
                    InlinedBoxedSlice::Huge { boxed } => &**boxed,
                }
            }

        }
    )
);

super_box! {
    { B16, 16 }, { B32, 32 }, { B48, 48 }, { B64, 64 }, { B80, 80 }, { B96, 96 }, { B112, 112 },
    { B128, 128 }, { B160, 160 }, { B192, 192 }, { B224, 224 }, { B256, 256 }, { B320, 320 },
    { B384, 384 }, { B448, 448 }, { B512, 512 }, { B640, 640 }, { B768, 768 }, { B896, 896 },
    { B1024, 1024 }, { B1280, 1280 }, { B1536, 1536 }, { B1792, 1792 }, { B2048, 2048 },
    { B2560, 2560 }, { B3072, 3072 }, { B3584, 3584 }, { B4096, 4096 }, { B5120, 5120 },
    { B6144, 6144 }, { B7168, 7168 }, { B8192, 8192 }, { B10240, 10240 }, { B12288, 12288 },
    { B14336, 14336 }, { B16384, 16384 }, { B20480, 20480 }, { B24576, 24576 }, { B28672, 28672 },
    { B32768, 32768 }, { B40960, 40960 }, { B49152, 49152 }, { B57344, 57344 }, { B65536, 65536 },
    { B81920, 81920 }, { B98304, 98304 }, { B114688, 114688 }
}

assert_eq_size!([u8; 16], InlinedBoxedSlice);
assert_eq_size!([u8; 16], Option<InlinedBoxedSlice>);

impl std::ops::Deref for InlinedBoxedSlice {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl From<&[u8]> for InlinedBoxedSlice {
    fn from(slice: &[u8]) -> Self {
        Self::from_slice(slice)
    }
}
