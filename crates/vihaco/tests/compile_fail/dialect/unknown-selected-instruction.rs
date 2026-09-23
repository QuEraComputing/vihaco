// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::dialect;

dialect! {
    arith {
        Add,
        Halt,
    }
}

#[allow(unused_macros)]
macro_rules! collect {
    ($($tokens:tt)*) => {};
}

arith::__private::__vihaco_instructions! {
    callback: collect,
    dialect: { arith },
    select: { Add, Missing },
    context: {},
}

fn main() {}
