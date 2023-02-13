use crate::types::SizedBytes;

use fuzzcheck::mutators::{
    fixed_len_vector::FixedLenVecMutator, integer::U8Mutator, map::MapMutator,
};
use fuzzcheck::MutatorWrapper;

type SizedBytesMutatorInner<const SIZE: usize> = MapMutator<
    Vec<u8>,
    SizedBytes<SIZE>,
    FixedLenVecMutator<u8, U8Mutator>,
    fn(&SizedBytes<SIZE>) -> Option<Vec<u8>>,
    fn(&Vec<u8>) -> SizedBytes<SIZE>,
    fn(&SizedBytes<SIZE>, f64) -> f64,
>;
pub struct SizedBytesMutator<const SIZE: usize>(SizedBytesMutatorInner<SIZE>);

#[no_coverage]
fn vec_from_sizedbytes<const SIZE: usize>(sb: &SizedBytes<SIZE>) -> Option<Vec<u8>> {
    Some(sb.0.to_vec())
}

#[no_coverage]
fn sizedbytes_from_vec<const SIZE: usize>(v: &Vec<u8>) -> SizedBytes<SIZE> {
    let bytes: [u8; SIZE] = v[0..SIZE].try_into().unwrap();
    SizedBytes::<SIZE>::from(bytes)
}

#[no_coverage]
fn complexity<const SIZE: usize>(_t: &SizedBytes<SIZE>, cplx: f64) -> f64 {
    cplx
}

impl<const SIZE: usize> SizedBytesMutator<SIZE> {
    #[no_coverage]
    pub fn new() -> Self {
        Self {
            0: MapMutator::new(
                FixedLenVecMutator::<u8, U8Mutator>::new_with_repeated_mutator(
                    U8Mutator::default(),
                    SIZE,
                ),
                vec_from_sizedbytes,
                sizedbytes_from_vec,
                complexity,
            ),
        }
    }
}

impl<const SIZE: usize> MutatorWrapper for SizedBytesMutator<SIZE> {
    type Wrapped = SizedBytesMutatorInner<SIZE>;

    fn wrapped_mutator(&self) -> &Self::Wrapped {
        &self.0
    }
}

impl<const SIZE: usize> Default for SizedBytesMutator<SIZE> {
    #[no_coverage]
    fn default() -> Self {
        SizedBytesMutator::<SIZE>::new()
    }
}
