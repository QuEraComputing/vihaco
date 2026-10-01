// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

pub struct Legacy;
impl rt::HasInstructionSet for Legacy {
    type Runtime = ();
    type Syntax = ();
}
impl rt::Component for Legacy {}
pub fn legacy_instruction_set<C: rt::Component>() -> Option<C::Runtime> {
    None
}
#[test]
fn legacy_contract_remains_available() {
    assert_eq!(legacy_instruction_set::<Legacy>(), None);
}
