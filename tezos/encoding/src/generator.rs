// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    cmp,
    fmt::Display,
    marker::PhantomData,
    ops::{Add, Bound, Div, Mul, RangeBounds, Rem, Shl, Shr, Sub},
};

use crate::encoding::Encoding;

pub trait GeneratorFactory {
    /// Generator for [bool] data
    fn bool(&mut self, field: &str) -> Box<dyn Generator<Item = bool>>;

    /// Generator for [u8] data.
    fn u8(&mut self, field: &str) -> Box<dyn Generator<Item = u8>>;

    /// Generator for [u16] data.
    fn u16(&mut self, field: &str) -> Box<dyn Generator<Item = u16>>;

    /// Generator for [u8] data.
    fn u32(&mut self, field: &str) -> Box<dyn Generator<Item = u32>>;

    /// Generator for [u16] data.
    fn u64(&mut self, field: &str) -> Box<dyn Generator<Item = u64>>;

    /// Generator for [u8] data.
    fn i8(&mut self, field: &str) -> Box<dyn Generator<Item = i8>>;

    /// Generator for [u16] data.
    fn i16(&mut self, field: &str) -> Box<dyn Generator<Item = i16>>;

    /// Generator for [u8] data.
    fn i32(&mut self, field: &str) -> Box<dyn Generator<Item = i32>>;

    /// Generator for [u16] data.
    fn i64(&mut self, field: &str) -> Box<dyn Generator<Item = i64>>;

    /// Generator for variable-length data size.
    fn size(
        &mut self,
        field: &str,
        list_encoding: Encoding,
        element_encoding: Encoding,
    ) -> Box<dyn Generator<Item = usize>>;

    /// Generator for string data.
    fn string(&mut self, field: &str, encoding: Encoding) -> Box<dyn Generator<Item = String>>;

    fn hash_bytes(
        &mut self,
        _field: &str,
        hash_type: HashType,
    ) -> Box<dyn Generator<Item = Vec<u8>>> {
        Box::new(value(vec![0; hash_type.size()]))
    }
}

pub use tezos_encoding_derive::Generated;

/// Trait for a type proviging an implementation of [Generator] for its values.
pub trait Generated {
    fn generator<F: GeneratorFactory>(prefix: &str, f: &mut F) -> Box<dyn Generator<Item = Self>>;
}

/// An interface for generating finite sequence of values of some type.
pub trait Generator {
    /// Type of generated values.
    type Item;

    /// Generates next value, if possible, and returns true.
    /// If no move values are available, returns `false` and sets the value to the initial value.
    fn next(&mut self) -> bool;

    /// Returns the current value.
    fn value(&self) -> Self::Item;

    /// Appends another generator to this one
    fn and<G: Generator<Item = Self::Item>>(self, other: G) -> And<Self, G, Self::Item>
    where
        Self: Sized,
    {
        And {
            g1: self,
            g2: other,
            first: true,
            phantom: PhantomData,
        }
    }

    /// Maps generator result using function `f`.
    fn map<T, F>(self, f: F) -> Map<Self, T, F>
    where
        Self: Sized,
        F: Fn(Self::Item) -> T,
    {
        Map {
            g: self,
            f,
            phantom: PhantomData,
        }
    }

    /// Iterator for all generated elements.
    fn iter(self) -> Iter<Self>
    where
        Self: Sized,
    {
        Iter {
            g: self,
            has_next: true,
        }
    }
}

impl<T: Generator + ?Sized> Generator for Box<T> {
    type Item = T::Item;

    fn next(&mut self) -> bool {
        self.as_mut().next()
    }

    fn value(&self) -> Self::Item {
        self.as_ref().value()
    }
}

#[derive(Debug, Clone)]
pub struct And<G1, G2, T> {
    g1: G1,
    g2: G2,
    first: bool,
    phantom: PhantomData<T>,
}

impl<G1, G2, T> Generator for And<G1, G2, T>
where
    G1: Generator<Item = T>,
    G2: Generator<Item = T>,
{
    type Item = T;

    fn next(&mut self) -> bool {
        if self.first {
            self.first = self.g1.next();
            true
        } else {
            self.first = !self.g2.next();
            !self.first
        }
    }

    fn value(&self) -> Self::Item {
        if self.first {
            self.g1.value()
        } else {
            self.g2.value()
        }
    }
}

#[derive(Debug, Clone)]
pub struct Map<G, T, F> {
    g: G,
    f: F,
    phantom: PhantomData<T>,
}

impl<G, T, F> Generator for Map<G, T, F>
where
    G: Generator,
    F: Fn(G::Item) -> T,
{
    type Item = T;

    fn next(&mut self) -> bool {
        self.g.next()
    }

    fn value(&self) -> Self::Item {
        (self.f)(self.g.value())
    }
}

pub struct Iter<G> {
    g: G,
    has_next: bool,
}

impl<G: Generator> Iterator for Iter<G> {
    type Item = G::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if self.has_next {
            let value = self.g.value();
            self.has_next = self.g.next();
            Some(value)
        } else {
            None
        }
    }
}

pub fn map<G, F, T1, T2>(g: G, f: F) -> Map<G, T2, F>
where
    G: Generator<Item = T1>,
    F: Fn(T1) -> T2,
{
    Map {
        g,
        f,
        phantom: PhantomData,
    }
}

#[derive(Debug, Clone)]
pub struct ValueGenerator<T>(T);

/// A value is a single-value [Generator] for its type.
impl<T: Clone> Generator for ValueGenerator<T> {
    type Item = T;

    fn next(&mut self) -> bool {
        false
    }

    fn value(&self) -> Self::Item {
        self.0.clone()
    }
}

pub fn value<T>(t: T) -> ValueGenerator<T> {
    ValueGenerator(t)
}

#[derive(Debug, Clone)]
pub struct ValuesGenerator<T> {
    values: Vec<T>,
    index: usize,
}

impl<T: Clone> Generator for ValuesGenerator<T> {
    type Item = T;

    fn next(&mut self) -> bool {
        self.index += 1;
        if self.index >= self.values.len() {
            self.index = 0;
            false
        } else {
            true
        }
    }

    fn value(&self) -> Self::Item {
        self.values[self.index].clone()
    }
}

/// Generator yelding specified values.
pub fn values<T: Clone>(vs: impl AsRef<[T]>) -> ValuesGenerator<T> {
    ValuesGenerator {
        values: vs.as_ref().to_vec(),
        index: 0,
    }
}

macro_rules! tuple_trait {
	($num1:tt $name1:ident, $num2:tt $name2:ident, $($num:tt $name:ident),*) => (
        tuple_trait!(__impl $num1 $name1, $num2 $name2; $($num $name),*);
    );
    (__impl $($num:tt $name:ident),+; $num1:tt $name1:ident, $($num2:tt $name2:ident),* ) => (
        tuple_trait_impl!($($num $name),*);
        tuple_trait!(__impl $($num $name),*, $num1 $name1; $($num2 $name2),*);
    );
    (__impl $($num:tt $name:ident),*; $num1:tt $name1:ident) => (
        tuple_trait_impl!($($num $name),*);
        tuple_trait_impl!($($num $name),*, $num1 $name1);
    );
}

macro_rules! tuple_trait_impl {
	($($num:tt $name:ident),+) => {
		impl<$($name),*> Tuple for ($($name),*)
        where
            $($name: Generator),*
        {
            type Item = ($($name::Item),*);

            fn next(&mut self) -> bool {
                $(self.$num.next())||*
            }

            fn value(&self) -> Self::Item {
                ($(self.$num.value()),*)
            }
        }
	};
}

tuple_trait!(
    0 G0,
    1 G1,
    2 G2,
    3 G3,
    4 G4,
    5 G5,
    6 G6,
    7 G7,
    8 G8,
    9 G9,
    10 G10,
    11 G11,
    12 G12,
    13 G13,
    14 G14,
    15 G15,
    16 G16,
    17 G17,
    18 G18,
    19 G19,
    20 G20,
    21 G21,
    22 G22,
    23 G23
);

pub trait Tuple: Sized {
    type Item;
    fn next(&mut self) -> bool;
    fn value(&self) -> Self::Item;
}

#[derive(Debug, Clone)]
pub struct TupleGenerator<T>(T);

impl<T: Tuple> Generator for TupleGenerator<T> {
    type Item = T::Item;

    fn next(&mut self) -> bool {
        self.0.next()
    }

    fn value(&self) -> Self::Item {
        self.0.value()
    }
}

pub fn tuple<T: Tuple>(t: T) -> TupleGenerator<T> {
    TupleGenerator(t)
}

pub fn compose<G1, G2, F, T>(g1: G1, g2: G2, f: F) -> Compose<G1, G2, F, T>
where
    G1: Generator,
    G2: Generator,
    F: Fn(G1::Item, G2::Item) -> T,
{
    Compose {
        g1,
        g2,
        f,
        phantom: PhantomData,
    }
}

#[derive(Debug, Clone)]
pub struct Compose<G1, G2, F, T> {
    g1: G1,
    g2: G2,
    f: F,
    phantom: PhantomData<T>,
}

impl<G1, G2, F, T> Generator for Compose<G1, G2, F, T>
where
    G1: Generator,
    G2: Generator,
    F: Fn(G1::Item, G2::Item) -> T,
{
    type Item = T;

    fn next(&mut self) -> bool {
        self.g1.next() || self.g2.next()
    }

    fn value(&self) -> Self::Item {
        (self.f)(self.g1.value(), self.g2.value())
    }
}

#[derive(Debug, Clone)]
pub struct IntGenerator<T> {
    min: T,
    max: T,
    step: T,
    radius: T,
    curr: T,
}

pub trait IntType:
    PartialOrd
    + Ord
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Rem<Output = Self>
    + Shl<Output = Self>
    + Shr<Output = Self>
    + Default
    + Copy
    + Display
{
    fn min_value() -> Self;
    fn max_value() -> Self;
    fn zero() -> Self;
    fn one() -> Self;
}

macro_rules! int_type {
    ($t:ty) => {
        impl IntType for $t {
            fn min_value() -> Self {
                <$t>::MIN
            }

            fn max_value() -> Self {
                <$t>::MAX
            }

            fn zero() -> Self {
                0
            }

            fn one() -> Self {
                1
            }
        }
    };
}

int_type!(u8);
int_type!(u16);
int_type!(u32);
int_type!(u64);
int_type!(usize);

fn decode_range_bounds<T: IntType>(bounds: impl RangeBounds<T>) -> (T, T) {
    let min = match bounds.start_bound() {
        Bound::Included(b) => *b,
        Bound::Excluded(b) => *b + T::one(),
        Bound::Unbounded => T::min_value(),
    };
    let max = match bounds.end_bound() {
        Bound::Included(b) => *b,
        Bound::Excluded(b) => *b - T::one(),
        Bound::Unbounded => T::max_value(),
    };
    (min, max)
}

impl<T> IntGenerator<T>
where
    T: IntType,
{
    pub fn new(min: T, max: T, step: T, radius: T) -> Self {
        assert!(min < max);
        assert!(step > T::zero());

        let step = cmp::min(step, max - min);
        let radius = cmp::min(radius, step >> T::one());
        Self {
            min,
            max,
            step,
            radius,
            curr: min,
        }
    }
}

impl<T> Generator for IntGenerator<T>
where
    T: IntType,
{
    type Item = T;

    fn next(&mut self) -> bool {
        if self.curr >= self.max {
            self.curr = self.min;
            return false;
        }
        let curr_offset = (self.curr - self.min) % self.step;
        if self.radius == curr_offset {
            let curr_node = (self.curr - self.min) / self.step;
            let curr_center = self.min + curr_node * self.step;
            let mut new_curr = if curr_center > self.max - self.step {
                self.max - self.radius
            } else {
                curr_center + self.step - self.radius
            };
            if new_curr <= self.curr {
                new_curr = self.curr + T::one();
            }
            self.curr = new_curr;
        } else {
            self.curr = self.curr + T::one();
        }
        true
    }

    fn value(&self) -> Self::Item {
        self.curr
    }
}

/// Helper composer for producing [std::vec::Vec] generator out of lenght generator and element generator
pub fn vec_of_items<G1, G2>(item: G1, len: G2) -> impl Generator<Item = Vec<G1::Item>>
where
    G1: Generator,
    G1::Item: Clone,
    G2: Generator<Item = usize>,
{
    compose(len, item, |len, item| vec![item; len])
}

/// Helper composer for producing [std::opt::Option] generator out of element generator
pub fn option<G1, G2>(item: G1, presence: G2) -> impl Generator<Item = Option<G1::Item>>
where
    G1: Generator,
    G2: Generator<Item = bool>,
{
    // TODO use optimized version that does not issue [None] multiple times.
    compose(
        presence,
        item,
        |presence, item| if presence { Some(item) } else { None },
    )
}

/// Generates all integers in the specified range.
pub fn full_range<T: IntType>(range: impl RangeBounds<T>) -> IntGenerator<T> {
    let (min, max) = decode_range_bounds(range);
    IntGenerator::new(min, max, T::one(), T::zero())
}

/// Generates some integers in the specified range.
///
/// Namely, `[min, min + 1, med - 1, med, med + 1, max - 1, max]`
pub fn some_in_range<T: IntType>(range: impl RangeBounds<T>) -> IntGenerator<T> {
    let (min, max) = decode_range_bounds(range);
    let step = ((max - min - T::one()) >> T::one()) + T::one();
    IntGenerator::new(min, max, step, T::one())
}

macro_rules! generated_hash {
    ($hash:ident) => {
        impl Generated for $hash {
            fn generator<F: GeneratorFactory>(
                field: &str,
                f: &mut F,
            ) -> Box<dyn Generator<Item = $hash>> {
                Box::new(
                    f.hash_bytes(field, $hash::hash_type())
                        .map(|bytes: Vec<u8>| $hash::try_from_bytes(bytes.as_slice()).unwrap()),
                )
            }
        }
    };
}

use crypto::hash::*;

generated_hash!(ChainId);
generated_hash!(BlockHash);
generated_hash!(BlockMetadataHash);
generated_hash!(OperationHash);
generated_hash!(OperationListListHash);
generated_hash!(OperationMetadataHash);
generated_hash!(OperationMetadataListListHash);
generated_hash!(ContextHash);
generated_hash!(ProtocolHash);
generated_hash!(ContractKt1Hash);
generated_hash!(ContractTz1Hash);
generated_hash!(ContractTz2Hash);
generated_hash!(ContractTz3Hash);
generated_hash!(CryptoboxPublicKeyHash);
generated_hash!(PublicKeyEd25519);
generated_hash!(PublicKeySecp256k1);
generated_hash!(PublicKeyP256);

#[cfg(test)]
mod test {
    use std::iter;

    use super::Generator;

    #[test]
    fn value() {
        let mut g = super::value(10);
        assert_eq!(g.value(), 10);
        assert!(!g.next());
        assert_eq!(g.value(), 10);
        assert!(!g.next());
        assert_eq!(g.value(), 10);
    }

    #[test]
    fn iter() {
        let g = super::value(10);
        let mut it = g.iter();
        assert_eq!(it.next(), Some(10));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn and() {
        let g = super::value(1).and(super::value(2));
        assert_eq!(g.iter().collect::<Vec<_>>(), vec![1, 2]);

        let g = super::values([1, 2, 3]).and(super::values([7, 8, 9]));
        assert_eq!(g.iter().collect::<Vec<_>>(), vec![1, 2, 3, 7, 8, 9]);
    }

    #[test]
    fn tuple() {
        let mut g = super::tuple((super::value(1), super::value(2)));
        assert_eq!(g.value(), (1, 2));
        assert!(!g.next())
    }

    #[test]
    fn full_range() {
        let g = super::full_range(0..10);
        let it = g.iter();
        let v = it.collect::<Vec<u8>>();
        assert_eq!(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9], v);

        let g = super::full_range(..);
        let it = g.iter();
        let v = it.collect::<Vec<u8>>();
        assert_eq!(v.len(), 256);
    }

    #[test]
    fn some_in_range() {
        let g = super::some_in_range(0..=10);
        let it = g.iter();
        let v = it.collect::<Vec<u8>>();
        assert_eq!(vec![0, 1, 4, 5, 6, 9, 10], v);

        let g = super::some_in_range(..);
        let it = g.iter();
        let v = it.collect::<Vec<u8>>();
        assert_eq!(v, vec![0, 1, 127, 128, 129, 254, 255]);

        let g = super::some_in_range(..);
        let it = g.iter();
        let v = it.collect::<Vec<u64>>();
        assert_eq!(v.len(), 7);

        let g = super::some_in_range(..=3);
        let it = g.iter();
        let v = it.collect::<Vec<u8>>();
        assert_eq!(v, vec![0, 1, 2, 3]);
    }

    #[test]
    fn map() {
        let g = super::full_range(0u16..=3).map(|i| i * 100);
        assert_eq!(g.iter().collect::<Vec<u16>>(), vec![0, 100, 200, 300]);
    }

    #[test]
    fn compose() {
        let g1 = super::values([1, 2]);
        let g2 = super::values(['a', 'b']);
        let values = super::compose(g1, g2, |l, c| iter::repeat(c).take(l).collect::<String>())
            .iter()
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 4);
        for e in ["a", "aa", "b", "bb"] {
            assert!(values.contains(&e.to_string()));
        }
    }
}
