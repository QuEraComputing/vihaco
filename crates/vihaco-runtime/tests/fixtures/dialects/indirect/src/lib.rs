// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![allow(dead_code)]

#[rt::dialect]
mod local {
    #[instruction]
    pub struct Halt;
}
struct Cpu;
impl rt::Component for Cpu {}
#[test]
fn indirect_feature_enables_complete_contract() {
    use rt::{Parse, chumsky::Parser};
    assert!(local::syntax::Halt::parser().parse("halt").has_output());
    assert_eq!(
        <producer::exported::Add as rt::__private::InstructionSpec>::MNEMONIC,
        "add"
    );
}
