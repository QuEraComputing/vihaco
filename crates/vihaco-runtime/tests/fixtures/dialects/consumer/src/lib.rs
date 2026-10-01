// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![allow(dead_code)]

use producer::exported as imported;
#[cfg(test)]
use rt::__private::{DialectSpec, InstructionSpec, require_coverage};
#[cfg(test)]
use rt::{Parse, chumsky::Parser};
struct Cpu;
impl rt::Component for Cpu {}
macro_rules! execute {
    ($($instruction:ty),*) => {$(
        impl rt::Execute<$instruction> for Cpu {
            type Message = (); type Effect = (); type Error = std::convert::Infallible;
            fn execute(&mut self, _: &$instruction, _: ()) -> Result<(), Self::Error> { Ok(()) }
        }
    )*};
}
execute!(imported::Add, imported::Halt);
#[test]
fn cross_crate_imports_preserve_identity_and_parser() {
    type Identity = <imported::Add as InstructionSpec>::Dialect;
    fn same<I: InstructionSpec<Dialect = Identity>>() {}
    same::<producer::arithmetic::Halt>();
    require_coverage::<Cpu, Identity>();
    assert_eq!(Identity::LEAVES[0].dialect, "arithmetic");
    assert_eq!(Identity::LEAVES[1].mnemonic, "halt");
    let value: <imported::Add as InstructionSpec>::Surface = imported::syntax::Add::parser()
        .parse("add -8")
        .into_result()
        .unwrap();
    assert_eq!(value.0, -8);
}
