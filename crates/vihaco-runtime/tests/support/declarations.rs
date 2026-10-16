// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime::__private::{
    Capabilities, Coverage, Description, DialectSpec, InstructionSpec, Leaf, Supports,
};
use vihaco_runtime::{Component, Effects, Execute};

pub struct A;
pub struct B;
pub struct Extra;
pub struct Dialect;
pub type RenamedDialect = Dialect;
pub struct Foreign;

impl DialectSpec for Dialect {
    const LEAVES: &'static [Leaf] = &[
        Leaf {
            dialect: "cpu",
            mnemonic: "a",
        },
        Leaf {
            dialect: "cpu",
            mnemonic: "b",
        },
    ];
}
impl DialectSpec for Foreign {
    const LEAVES: &'static [Leaf] = &[Leaf {
        dialect: "extra",
        mnemonic: "extra",
    }];
}
impl<C: Execute<A> + Execute<B>> Coverage<C> for Dialect {}
impl<C: Execute<Extra>> Coverage<C> for Foreign {}

impl InstructionSpec for A {
    type Dialect = Dialect;
    const MNEMONIC: &'static str = "a";
}
impl InstructionSpec for B {
    type Dialect = Dialect;
    const MNEMONIC: &'static str = "b";
}
impl InstructionSpec for Extra {
    type Dialect = Foreign;
    const MNEMONIC: &'static str = "extra";
}

pub struct Cpu<'a, const N: usize = 2>(pub &'a [u8; N]);
impl<const N: usize> Component for Cpu<'_, N> {}
impl<const N: usize> Execute<A> for Cpu<'_, N> {
    type Message = usize;
    type Effect = usize;
    type Error = eyre::Report;
    fn execute(&mut self, _: &A, input: usize) -> eyre::Result<usize> {
        Ok(input + self.0.len())
    }
}
impl<const N: usize> Execute<B> for Cpu<'_, N> {
    type Message = ();
    type Effect = Effects<usize>;
    type Error = eyre::Report;
    fn execute(&mut self, _: &B, _: ()) -> eyre::Result<Self::Effect> {
        Ok(Effects::one(N))
    }
}
impl<const N: usize> Execute<Extra> for Cpu<'_, N> {
    type Message = ();
    type Effect = ();
    type Error = eyre::Report;
    fn execute(&mut self, _: &Extra, _: ()) -> eyre::Result<()> {
        Ok(())
    }
}
impl<const N: usize> Supports<RenamedDialect> for Cpu<'_, N> {}
impl<const N: usize> Capabilities for Cpu<'_, N> {
    const DESCRIPTION: &'static Description = &Description::Component(&[Dialect::LEAVES]);
}
