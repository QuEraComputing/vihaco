// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::{FromBytes, WriteBytes};

pub trait ByteWidth {
    fn width() -> u32;
}

pub trait OpCode {
    fn opcode(&self) -> u8;
}

pub trait Instruction: Sized + ByteWidth + OpCode + FromBytes + WriteBytes {}

impl<T> Instruction for T where T: Sized + ByteWidth + OpCode + FromBytes + WriteBytes {}

macro_rules! impl_builtin_width {
    ($ty:ty, $width:expr) => {
        impl ByteWidth for $ty {
            fn width() -> u32 {
                $width
            }
        }
    };
}

impl_builtin_width!(u32, 4);
impl_builtin_width!(u64, 8);
impl_builtin_width!(i64, 8);
impl_builtin_width!(f64, 8);
impl_builtin_width!(bool, 1);
impl_builtin_width!((), 0);
