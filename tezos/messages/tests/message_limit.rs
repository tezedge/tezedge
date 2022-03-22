// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![allow(dead_code)]

use std::{
    cmp::{max, min},
    fmt, ops,
};

use tezos_encoding::encoding::Encoding;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Limit {
    Fixed(usize),
    UpTo(usize),
    Var,
}

#[derive(PartialEq, Eq, Clone, Copy, Default)]
pub struct Limits(usize, Limit);

impl Limit {
    pub fn is_limited(&self) -> bool {
        !matches!(self, Limit::Var)
    }

    pub fn is_variable_length(&self) -> bool {
        !matches!(self, Limit::Fixed(_))
    }

    pub fn union(self, other: Self) -> Self {
        match (self, other) {
            (Limit::Fixed(a), Limit::Fixed(b)) => Limit::Fixed(max(a, b)),
            (Limit::Fixed(a), Limit::UpTo(b))
            | (Limit::UpTo(a), Limit::Fixed(b))
            | (Limit::UpTo(a), Limit::UpTo(b)) => Limit::UpTo(max(a, b)),
            _ => Limit::Var,
        }
    }

    fn restrict(self, max: usize) -> Self {
        match self {
            Limit::Fixed(a) => {
                assert!(a <= max, "cannot restrict fixed size {} to {}", a, max);
                self
            }
            Limit::UpTo(a) => Limit::UpTo(min(a, max)),
            Limit::Var => Limit::UpTo(max),
        }
    }
}

impl Limits {
    pub fn is_limited(&self) -> bool {
        !matches!(self.1, Limit::Var)
    }

    pub fn union(self, other: Self) -> Self {
        match (self.1, other.1) {
            (Limit::Fixed(a), Limit::Fixed(b)) => {
                Self(min(self.0, other.0), Limit::Fixed(max(a, b)))
            }
            (Limit::Fixed(a), Limit::UpTo(b))
            | (Limit::UpTo(a), Limit::Fixed(b))
            | (Limit::UpTo(a), Limit::UpTo(b)) => {
                Self(min(self.0, other.0), Limit::UpTo(max(a, b)))
            }
            _ => Self(min(self.0, other.0), Limit::Var),
        }
    }

    fn restrict(self, max: usize) -> Self {
        match self.1 {
            Limit::Fixed(a) => {
                assert!(a <= max, "cannot restrict fixed size {} to {}", a, max);
                self
            }
            Limit::UpTo(a) => Self(self.0, Limit::UpTo(min(a, max))),
            Limit::Var => Self(self.0, Limit::UpTo(max)),
        }
    }

    pub fn lower(&self) -> usize {
        self.0
    }

    pub fn upper(&self) -> &Limit {
        &self.1
    }
}

impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Limit::Fixed(size) => write!(f, "{}", size),
            Limit::UpTo(size) => write!(f, "up to {}", size),
            Limit::Var => write!(f, "variable"),
        }
    }
}

impl Default for Limit {
    fn default() -> Self {
        0.into()
    }
}

impl ops::Add for Limit {
    type Output = Limit;
    fn add(self, other: Self) -> Self {
        match (self, other) {
            (Limit::Fixed(a), Limit::Fixed(b)) => Limit::Fixed(a + b),
            (Limit::Fixed(a), Limit::UpTo(b))
            | (Limit::UpTo(a), Limit::Fixed(b))
            | (Limit::UpTo(a), Limit::UpTo(b)) => Limit::UpTo(a + b),
            _ => Limit::Var,
        }
    }
}

impl ops::Add<usize> for Limit {
    type Output = Limit;
    fn add(self, other: usize) -> Self {
        match self {
            Limit::Fixed(a) => Limit::Fixed(a + other),
            Limit::UpTo(a) => Limit::UpTo(a + other),
            _ => Limit::Var,
        }
    }
}

impl ops::Add<usize> for Limits {
    type Output = Limits;
    fn add(self, other: usize) -> Self {
        match self.1 {
            Limit::Fixed(a) => Self(self.0 + other, Limit::Fixed(a + other)),
            Limit::UpTo(a) => Self(self.0 + other, Limit::UpTo(a + other)),
            _ => Self(self.0 + other, Limit::Var),
        }
    }
}

impl ops::AddAssign<usize> for Limit {
    fn add_assign(&mut self, other: usize) {
        match self {
            Limit::Fixed(a) | Limit::UpTo(a) => *a += other,
            _ => (),
        }
    }
}

impl ops::AddAssign for Limit {
    fn add_assign(&mut self, other: Limit) {
        let s: &mut Self = self;
        match (s, other) {
            (Limit::Fixed(a), Limit::Fixed(b)) => *a += b,
            (Limit::Fixed(a), Limit::UpTo(b)) => *self = Limit::UpTo(*a + b),
            (Limit::UpTo(a), Limit::Fixed(b)) | (Limit::UpTo(a), Limit::UpTo(b)) => *a += b,
            (s, _) => *s = Limit::Var,
        }
    }
}

impl ops::AddAssign for Limits {
    fn add_assign(&mut self, other: Limits) {
        self.0 += other.0;
        self.1 += other.1;
    }
}

impl ops::Mul for Limit {
    type Output = Limit;
    fn mul(self, other: Self) -> Self {
        match (self, other) {
            (Limit::Fixed(a), Limit::Fixed(b)) => Limit::Fixed(a * b),
            (Limit::Fixed(a), Limit::UpTo(b))
            | (Limit::UpTo(a), Limit::Fixed(b))
            | (Limit::UpTo(a), Limit::UpTo(b)) => Limit::UpTo(a * b),
            _ => Limit::Var,
        }
    }
}

impl ops::Mul<Limit> for Limits {
    type Output = Limits;
    fn mul(self, other: Limit) -> Self {
        match (self.1, other) {
            (Limit::Fixed(a), Limit::Fixed(b)) => Self(self.0 * b, Limit::Fixed(a * b)),
            (Limit::Fixed(a), Limit::UpTo(b)) | (Limit::UpTo(a), Limit::UpTo(b)) => {
                Self(0, Limit::UpTo(a * b))
            }
            (Limit::UpTo(a), Limit::Fixed(b)) => Self(self.0 * b, Limit::UpTo(a * b)),
            _ => Self(0, Limit::Var),
        }
    }
}

impl ops::Mul<usize> for Limit {
    type Output = Limit;
    fn mul(self, other: usize) -> Self {
        match self {
            Limit::Fixed(a) => Limit::Fixed(a * other),
            Limit::UpTo(a) => Limit::UpTo(a * other),
            _ => Limit::Var,
        }
    }
}

impl ops::Mul<&usize> for Limit {
    type Output = Limit;
    fn mul(self, other: &usize) -> Self {
        match self {
            Limit::Fixed(a) => Limit::Fixed(a * *other),
            Limit::UpTo(a) => Limit::UpTo(a * *other),
            _ => Limit::Var,
        }
    }
}

impl From<usize> for Limit {
    fn from(source: usize) -> Self {
        Limit::Fixed(source)
    }
}

impl From<&usize> for Limit {
    fn from(source: &usize) -> Self {
        Limit::Fixed(*source)
    }
}

impl From<usize> for Limits {
    fn from(source: usize) -> Self {
        Self(source, Limit::Fixed(source))
    }
}

impl From<&usize> for Limits {
    fn from(source: &usize) -> Self {
        Self(*source, Limit::Fixed(*source))
    }
}

impl From<Limit> for Limits {
    fn from(limit: Limit) -> Self {
        Self(0, limit)
    }
}

pub fn get_max_size(encoding: &Encoding) -> Limit {
    get_limits(encoding).1
}

pub fn get_min_size(encoding: &Encoding) -> usize {
    get_limits(encoding).0
}

/// Returns (min, max) limits for the encoding.
pub fn get_limits(encoding: &Encoding) -> Limits {
    use Encoding::*;
    use Limit::Var;
    match encoding {
        Unit => 0.into(),
        Int8 | Uint8 | Bool => 1.into(),
        Int16 | Uint16 => 2.into(),
        Int31 | Int32 | Uint32 => 4.into(),
        Int64 | RangedInt | Float | Timestamp => 8.into(),
        RangedFloat => 16.into(),
        Z | Mutez => Var.into(),
        Hash(hash) => hash.size().into(),
        String => Var.into(),
        BoundedString(max) => Limit::UpTo(*max).into(),
        Bytes => Var.into(),
        Tags(size, map) => {
            let mut max = Limits::default();
            for tag in map.tags() {
                let size = get_limits(tag.get_encoding());
                max = max.union(size);
            }
            max + *size
        }
        List(_) => Var.into(),
        BoundedList(max, encoding) => {
            let element_size = get_limits(encoding);
            element_size * Limit::UpTo(*max)
        }
        Enum => 1.into(),
        Option(encoding) => get_limits(encoding) + 1,
        OptionalField(encoding) => get_limits(encoding) + 1,
        Obj(_, fields) => {
            let mut sum = 0.into();
            for field in fields {
                let size = get_limits(field.get_encoding());
                sum += size;
            }
            sum
        }
        Dynamic(encoding) => {
            let size = get_limits(encoding);
            size + 4
        }
        BoundedDynamic(max, encoding) => {
            let size = get_limits(encoding);
            size.restrict(*max) + 4
        }
        Sized(fixed_size, encoding) => {
            let _size = get_limits(encoding);
            fixed_size.into()
        }
        Bounded(bounded_size, encoding) => {
            let _size = get_limits(encoding);
            Limit::UpTo(*bounded_size).into()
        }
        Custom => Limit::UpTo(100).into(), // 3 hashes, three left/right tags, one op tag, 3 * (32 + 1) + 1
        _ => unimplemented!(),
    }
}
