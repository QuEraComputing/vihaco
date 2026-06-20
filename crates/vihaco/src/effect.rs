// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use smallvec::{SmallVec, smallvec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effects<T> {
    None,
    One(T),
    Many(SmallVec<[T; 2]>),
}

impl<T> Effects<T> {
    pub fn none() -> Self {
        Self::None
    }

    pub fn one(value: T) -> Self {
        Self::One(value)
    }

    pub fn many(values: SmallVec<[T; 2]>) -> Self {
        match values.len() {
            0 => Self::None,
            1 => Self::One(values.into_iter().next().unwrap()),
            _ => Self::Many(values),
        }
    }

    pub fn append(self, effect: T) -> Self {
        match self {
            Self::None => Self::One(effect),
            Self::One(first) => Self::Many(smallvec![first, effect]),
            Self::Many(mut values) => {
                values.push(effect);
                Self::Many(values)
            }
        }
    }

    pub fn extend(self, other: Self) -> Self {
        match (self, other) {
            (Self::None, rhs) => rhs,
            (lhs, Self::None) => lhs,
            (Self::One(a), Self::One(b)) => Self::Many(smallvec![a, b]),
            (Self::One(a), Self::Many(mut bs)) => {
                let mut out = smallvec![a];
                out.append(&mut bs);
                Self::Many(out)
            }
            (Self::Many(mut as_), Self::One(b)) => {
                as_.push(b);
                Self::Many(as_)
            }
            (Self::Many(mut as_), Self::Many(mut bs)) => {
                as_.append(&mut bs);
                Self::Many(as_)
            }
        }
    }

    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> Effects<U> {
        match self {
            Self::None => Effects::None,
            Self::One(value) => Effects::One(f(value)),
            Self::Many(values) => Effects::many(values.into_iter().map(f).collect()),
        }
    }

    pub fn flat_map<U>(self, mut f: impl FnMut(T) -> Effects<U>) -> Effects<U> {
        let mut out = SmallVec::<[U; 2]>::new();
        for value in self {
            match f(value) {
                Effects::None => {}
                Effects::One(value) => out.push(value),
                Effects::Many(mut values) => out.append(&mut values),
            }
        }
        Effects::many(out)
    }
}

impl<T> From<Option<T>> for Effects<T> {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => Self::One(value),
            None => Self::None,
        }
    }
}

impl<T> From<Vec<T>> for Effects<T> {
    fn from(values: Vec<T>) -> Self {
        Self::many(values.into_iter().collect())
    }
}

impl<T> From<SmallVec<[T; 2]>> for Effects<T> {
    fn from(values: SmallVec<[T; 2]>) -> Self {
        Self::many(values)
    }
}

impl<T> IntoIterator for Effects<T> {
    type Item = T;
    type IntoIter = smallvec::IntoIter<[T; 2]>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::None => SmallVec::new().into_iter(),
            Self::One(value) => smallvec![value].into_iter(),
            Self::Many(values) => values.into_iter(),
        }
    }
}
