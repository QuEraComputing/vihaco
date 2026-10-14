// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[rt::dialect]
pub mod arithmetic {
    use std::primitive::i64 as Signed;
    /// Add an integer operand.
    #[instruction]
    #[pattern = "'add $0:Signed"]
    pub struct Add(pub i64);

    #[instruction]
    pub struct Halt;
}
pub use arithmetic as exported;

#[rt::component]
#[instructions { crate::arithmetic }]
pub struct Cpu;

impl<I> rt::Execute<I> for Cpu {
    type Message = ();

    type Effect = ();

    type Error = std::convert::Infallible;

    fn execute(&mut self, _: &I, _: ()) -> Result<(), Self::Error> { Ok(()) }
}

#[rt::composite]
pub struct Machine {
    #[allow(dead_code)]
    #[device(42, alias = "host")]
    cpu: Cpu,
}

#[rt::dialect]
mod private_control {
    #[instruction]
    pub struct Stop;
}
pub use private_control::Stop as PrivateStop;

#[rt::component]
#[instructions { private_control }]
pub struct PrivateDevice;

impl rt::Execute<PrivateStop> for PrivateDevice {
    type Message = (); type Effect = (); type Error = std::convert::Infallible;

    fn execute(&mut self, _: &PrivateStop, _: ()) -> Result<(), Self::Error> { Ok(()) }
}

impl<I> rt::Step<rt::field!(Machine::cpu), I> for Machine
where Cpu: rt::Execute<I, Message = ()> {
    fn step(&mut self, instruction: I) { assert!(rt::Execute::execute(&mut self.cpu, &instruction, ()).is_ok()); }
}
