//! This module contains varius recorder implementations.
//!
//! Each recorder has two methods:
//! - `record(&mut self) -> ...`, which will return value, which can be
//!   used as a substitute for a given type, for which we implement recorder.
//!   In case if it's a simple type that implements `Clone`, `record`
//!   and `finish_recording` simply return cloned value. Otherwise
//!   value is returned usage of which will be intercepted and accumulated.
//!
//! - `finish_recording(self) -> RecordedValue`, which will finish
//!   recording and return recorded value.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crate::DefaultRecorder;

mod clone_recorder;
pub use clone_recorder::*;

mod iterator_recorder;
pub use iterator_recorder::*;

mod stream_recorder;
pub use stream_recorder::*;

macro_rules! impl_default_recorder_for_simple_clonable {
    ($name:ident) => {
        impl DefaultRecorder for $name {
            type Recorder = CloneRecorder<$name>;

            fn default_recorder(self) -> Self::Recorder {
                CloneRecorder::new(self)
            }
        }
    };
}

impl_default_recorder_for_simple_clonable!(u8);
impl_default_recorder_for_simple_clonable!(u16);
impl_default_recorder_for_simple_clonable!(u32);
impl_default_recorder_for_simple_clonable!(u64);
impl_default_recorder_for_simple_clonable!(u128);
impl_default_recorder_for_simple_clonable!(usize);

impl_default_recorder_for_simple_clonable!(i8);
impl_default_recorder_for_simple_clonable!(i16);
impl_default_recorder_for_simple_clonable!(i32);
impl_default_recorder_for_simple_clonable!(i64);
impl_default_recorder_for_simple_clonable!(i128);
impl_default_recorder_for_simple_clonable!(isize);

impl_default_recorder_for_simple_clonable!(String);

impl_default_recorder_for_simple_clonable!(Instant);
impl_default_recorder_for_simple_clonable!(Duration);

impl_default_recorder_for_simple_clonable!(SocketAddr);
