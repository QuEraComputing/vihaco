// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[rt::dialect]
pub mod arithmetic {
    /// Add an integer operand.
    #[instruction]
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
