// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "typed-declarations")]

#[vihaco::dialect]
mod typed_control {
    #[instruction]
    pub struct Halt;
}

#[vihaco::component]
#[instructions { typed_control }]
struct TypedCpu;

impl vihaco::Execute<typed_control::Halt> for TypedCpu {
    type Message = ();
    type Effect = ();
    type Error = std::convert::Infallible;
    fn execute(&mut self, _: &typed_control::Halt, _: ()) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn typed_component_is_exported_at_the_crate_root() {
    vihaco::__private::require_available::<TypedCpu, typed_control::Halt>();
}
