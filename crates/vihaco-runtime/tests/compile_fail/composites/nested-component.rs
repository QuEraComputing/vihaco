// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime::{component, composite, dialect, field, Execute};
#[dialect] mod control { #[instruction] pub struct Halt; }
#[component] #[instructions { control }] struct Cpu;
impl Execute<control::Halt> for Cpu {
    type Message = (); type Effect = (); type Error = std::convert::Infallible;
    fn execute(&mut self, _: &control::Halt, _: ()) -> Result<(), Self::Error> { Ok(()) }
}
#[composite] struct Parent { #[device] cpu: Cpu }
fn main() { let _: Option<field!(Parent::cpu::missing)> = None; }
