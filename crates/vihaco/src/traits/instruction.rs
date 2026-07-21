// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::{FromBytes, WriteBytes};

pub trait OpCode {
    fn width() -> u32;
    fn opcode(&self) -> u8;
}

pub trait Instruction: Sized + OpCode + FromBytes + WriteBytes {}

impl<T> Instruction for T where T: Sized + OpCode + FromBytes + WriteBytes {}

macro_rules! impl_scalar_opcode {
    ($ty:ty, $width:expr) => {
        impl OpCode for $ty {
            fn width() -> u32 {
                $width
            }

            fn opcode(&self) -> u8 {
                0
            }
        }
    };
}

impl_scalar_opcode!(u32, 4);
impl_scalar_opcode!(u64, 8);
impl_scalar_opcode!(i64, 8);
impl_scalar_opcode!(f64, 8);

impl OpCode for bool {
    fn width() -> u32 {
        1
    }

    fn opcode(&self) -> u8 {
        0
    }
}

impl OpCode for () {
    fn width() -> u32 {
        0
    }

    fn opcode(&self) -> u8 {
        0
    }
}
