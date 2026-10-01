// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![allow(dead_code)]

#[cfg(test)]
use vm::{Parse, chumsky::Parser};
#[vm::dialect]
pub mod arithmetic {
    #[instruction]
    pub struct Add(pub i64);
}
mod custom {
    pub use vm::*;
}
#[vm::dialect]
#[vihaco(crate = crate::custom)]
mod custom_root {
    #[instruction]
    pub struct Halt;
}
#[test]
fn facade_has_a_complete_parser_stack() {
    let value: <arithmetic::Add as vm::__private::InstructionSpec>::Surface =
        arithmetic::syntax::Add::parser()
            .parse("add 9")
            .into_result()
            .unwrap();
    assert_eq!(value.0, 9);
    assert!(
        custom_root::syntax::Halt::parser()
            .parse("halt")
            .has_output()
    );
}
