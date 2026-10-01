// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![allow(dead_code)]

#[rt::dialect]
mod local {
    #[instruction]
    pub struct Halt;
}
#[rt::component]
#[instructions { local }]
struct Cpu;
impl rt::Execute<local::Halt> for Cpu {
    type Message = (); type Effect = (); type Error = std::convert::Infallible;
    fn execute(&mut self, _: &local::Halt, _: ()) -> Result<(), Self::Error> { Ok(()) }
}
#[test]
fn indirect_feature_enables_complete_contract() {
    use rt::{Parse, chumsky::Parser};
    assert!(local::syntax::Halt::parser().parse("halt").has_output());
    rt::__private::require_available::<Cpu, local::Halt>();
    assert_eq!(
        <producer::exported::Add as rt::__private::InstructionSpec>::MNEMONIC,
        "add"
    );
}
