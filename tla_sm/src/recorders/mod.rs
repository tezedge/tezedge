pub mod clone_recorder;
pub mod stream_recorder;

use std::time::{Duration, Instant};

use crate::DefaultRecorder;
use clone_recorder::CloneRecorder;

macro_rules! impl_default_recorder_for_simple_clonable {
    ($name:ident) => {
        impl<'a> DefaultRecorder for $name {
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

impl_default_recorder_for_simple_clonable!(i8);
impl_default_recorder_for_simple_clonable!(i16);
impl_default_recorder_for_simple_clonable!(i32);
impl_default_recorder_for_simple_clonable!(i64);
impl_default_recorder_for_simple_clonable!(i128);

impl_default_recorder_for_simple_clonable!(String);

impl_default_recorder_for_simple_clonable!(Instant);
impl_default_recorder_for_simple_clonable!(Duration);
