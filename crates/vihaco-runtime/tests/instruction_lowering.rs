// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "derive")]

use std::collections::HashMap;
use vihaco_runtime::{
    Dispatch, Execute, HasInstructionSet, Lower, Parse, Step, Syntax, chumsky::Parser, component,
    composite, dialect, field,
};

trait FunctionLookup {
    fn function_index(&self, name: &str) -> eyre::Result<u32>;
}

trait StringIntern {
    fn intern(&mut self, text: &str) -> u32;
}

#[dialect]
mod symbolic {
    use vihaco_parser::{Ident, QuotedString};

    #[instruction]
    #[pattern = "'call $0:Ident `,` $1"]
    pub struct Call(pub u32, pub u32);

    #[instruction]
    #[pattern = "'named $1:QuotedString `,` $0:Ident"]
    pub struct Named(pub u32, pub u32);

    #[instruction]
    #[pattern = "'load $0:u32"]
    pub struct Load(pub u32);

    #[instruction]
    pub struct Halt;
}

impl<C: FunctionLookup + ?Sized> Lower<C> for symbolic::Call<Syntax> {
    type Runtime = symbolic::Call;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        Ok(symbolic::Call(
            context.function_index(self.0.as_str())?,
            self.1,
        ))
    }
}

impl<C: FunctionLookup + StringIntern + ?Sized> Lower<C> for symbolic::Named<Syntax> {
    type Runtime = symbolic::Named;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        Ok(symbolic::Named(
            context.function_index(self.0.as_str())?,
            context.intern(self.1.as_str()),
        ))
    }
}

#[component]
#[instructions { symbolic }]
struct Cpu;

impl<I> Execute<I> for Cpu {
    type Message = ();
    type Effect = ();
    type Error = std::convert::Infallible;
    fn execute(&mut self, _: &I, _: ()) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[composite]
struct Child {
    #[device(1, alias = "host")]
    cpu: Cpu,
}

impl<I> Step<field!(Child::cpu), I> for Child {
    fn step(&mut self, instruction: &I, _: &()) -> vihaco_runtime::eyre::Result<()> {
        self.cpu.execute(instruction, ())?;
        Ok(())
    }
}

#[composite]
struct Machine {
    #[device(2)]
    child: Child,
    #[device(3)]
    spare: Cpu,
    executed: usize,
}

impl<I> Step<field!(Machine::child::cpu), I> for Machine {
    fn step(&mut self, instruction: &I, _: &()) -> vihaco_runtime::eyre::Result<()> {
        self.child.cpu.execute(instruction, ())?;
        self.executed += 1;
        Ok(())
    }
}

impl<I> Step<field!(Machine::spare), I> for Machine {
    fn step(&mut self, instruction: &I, _: &()) -> vihaco_runtime::eyre::Result<()> {
        self.spare.execute(instruction, ())?;
        self.executed += 10;
        Ok(())
    }
}

type Source = <Machine as HasInstructionSet>::Syntax;

type Runtime = <Machine as HasInstructionSet>::Runtime;

#[derive(Default)]
struct Loader {
    functions: HashMap<String, u32>,
    strings: Vec<String>,
    code: Vec<Runtime>,
}

impl FunctionLookup for Loader {
    fn function_index(&self, name: &str) -> eyre::Result<u32> {
        self.functions
            .get(name)
            .copied()
            .ok_or_else(|| eyre::eyre!("unknown function {name}"))
    }
}

impl StringIntern for Loader {
    fn intern(&mut self, text: &str) -> u32 {
        let index = self.strings.len() as u32;
        self.strings.push(text.to_owned());
        index
    }
}

#[test]
fn custom_context_lowers_aliased_and_nested_instructions() {
    let mut loader = Loader {
        functions: HashMap::from([("entry".to_owned(), 7)]),
        ..Loader::default()
    };
    let mut machine = Machine {
        child: Child { cpu: Cpu },
        spare: Cpu,
        executed: 0,
    };
    let source = [
        "child::host::symbolic::call entry, 2",
        "spare::symbolic::named \"hello\", entry",
        "spare::symbolic::load 42",
        "child::cpu::symbolic::halt",
    ];

    let code = source.map(|line| {
        Source::parser()
            .parse(line)
            .into_result()
            .unwrap()
            .lower(&mut loader)
            .unwrap()
    });

    assert_eq!(loader.strings, ["hello"]);
    assert_eq!(
        code,
        [
            __vihaco_MachineInstruction::V_child__cpu__symbolic__Call(symbolic::Call(7, 2)),
            __vihaco_MachineInstruction::V_spare__symbolic__Named(symbolic::Named(7, 0)),
            __vihaco_MachineInstruction::V_spare__symbolic__Load(symbolic::Load(42)),
            __vihaco_MachineInstruction::V_child__cpu__symbolic__Halt(symbolic::Halt),
        ]
    );
    for instruction in &code {
        machine.dispatch(instruction, &()).unwrap();
    }
    assert_eq!(machine.executed, 22);
}

#[test]
fn syntax_is_retained_and_lowering_errors_do_not_replace_loaded_code() {
    let source = symbolic::Call::<Syntax>::parser()
        .parse("call missing, 2")
        .into_result()
        .unwrap();
    assert_eq!(source.0.as_str(), "missing");
    let mut loader = Loader {
        code: vec![__vihaco_MachineInstruction::V_spare__symbolic__Halt(
            symbolic::Halt,
        )],
        ..Loader::default()
    };
    let source = Source::parser()
        .parse("child::cpu::symbolic::call missing, 2")
        .into_result()
        .unwrap();
    assert!(
        source
            .lower(&mut loader)
            .unwrap_err()
            .to_string()
            .contains("unknown function missing")
    );
    assert_eq!(loader.code.len(), 1);
    // Equal annotations and unannotated instructions remain nongeneric and can
    // lower against a context with no capabilities at all.
    assert_eq!(symbolic::Load(9).lower(&mut ()).unwrap(), symbolic::Load(9));
    assert_eq!(symbolic::Halt.lower(&mut ()).unwrap(), symbolic::Halt);
}

#[test]
fn lowering_capability_diagnostics() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/lowering/*.rs");
}
