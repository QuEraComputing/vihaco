// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{
    Apply, BuildMessage, ReusableMessageRecipe, Execute, ExecutionState, HasInstructionSet, Image, Loadable, NoContext,
    PopFromStack, PushToStack, SstFile, Step, Type, Value, component, composite, dialect, field,
    loader::MetadataLoweringContext,
    module::{LocalModule, ProgramMetadata},
    syntax::{HeaderDeclarations, ParsedModule, lower_code_with_metadata},
};

#[dialect]
pub mod arithmetic {
    #[instruction]
    pub struct Add;
}

#[component]
#[instructions { arithmetic }]
#[derive(Default)]
pub struct Calculator {
    additions: usize,
}

impl Execute<arithmetic::Add> for Calculator {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &arithmetic::Add,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let sum = lhs
            .checked_add(rhs)
            .ok_or_else(|| eyre::eyre!("addition overflow"))?;
        self.additions += 1;
        Ok(PushToStack((sum,)))
    }
}

#[composite]
pub struct Machine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    calculator: Calculator,
}

type Instruction = <Machine as HasInstructionSet>::Runtime;

type SourceInstruction = <Machine as HasInstructionSet>::Syntax;

type Program = LocalModule<Instruction, Value, Type>;

impl<I, M> BuildMessage<field!(Machine::execution), I, Program, M> for Machine
where
    ExecutionState: Execute<I, Message = M>,
    M: ReusableMessageRecipe<Self, field!(Machine::execution), I, Program>,
{
    fn build_message(&mut self, instruction: &I, context: &Program) -> eyre::Result<M> {
        <M as ReusableMessageRecipe<Self, field!(Machine::execution), I, Program>>::prepare(
            self, instruction, context,
        )
    }
}

impl<I, M> BuildMessage<field!(Machine::calculator), I, Program, M> for Machine
where
    Calculator: Execute<I, Message = M>,
    M: ReusableMessageRecipe<Self, field!(Machine::calculator), I, Program>,
{
    fn build_message(&mut self, instruction: &I, context: &Program) -> eyre::Result<M> {
        <M as ReusableMessageRecipe<Self, field!(Machine::calculator), I, Program>>::prepare(
            self, instruction, context,
        )
    }
}

impl<I> Step<field!(Machine::execution), I, Program> for Machine
where
    ExecutionState: Execute<I, Effect = (), Error = eyre::Report>,
    Self: BuildMessage<field!(Machine::execution), I, Program>,
{
    fn step(&mut self, instruction: &I, module: &Program) -> eyre::Result<()> {
        let message =
            <Self as BuildMessage<field!(Machine::execution), I, Program>>::build_message(
                self,
                instruction,
                module,
            )?;
        self.execution.execute(instruction, message)
    }
}

impl<I> Step<field!(Machine::calculator), I, Program> for Machine
where
    Calculator: Execute<I, Error = eyre::Report>,
    Self: BuildMessage<field!(Machine::calculator), I, Program>,
    ExecutionState: Apply<<Calculator as Execute<I>>::Effect>,
{
    fn step(&mut self, instruction: &I, module: &Program) -> eyre::Result<()> {
        let message =
            <Self as BuildMessage<field!(Machine::calculator), I, Program>>::build_message(
                self,
                instruction,
                module,
            )?;
        let effect = self.calculator.execute(instruction, message)?;
        self.execution.apply(effect);
        Ok(())
    }
}

pub fn main() -> eyre::Result<()> {
    let source = include_str!("first-machine.sst");
    let file = SSTFile::<NoContext>::from_text(source)?;
    let section = file
        .section("main")
        .ok_or_else(|| eyre::eyre!("missing main section"))?;
    let ParsedModule {
        header,
        code,
        symbols,
    } = ParsedModule::<
        SourceInstruction,
        HeaderDeclarations<ProgramMetadata>,
    >::parse_section(section)?;

    let mut declarations = header.into_iter();
    let metadata = declarations
        .next()
        .ok_or_else(|| eyre::eyre!("missing program metadata"))?;
    eyre::ensure!(
        declarations.next().is_none(),
        "expected one metadata declaration"
    );
    let mut context = MetadataLoweringContext::new(&metadata, &symbols)?;
    let code = lower_code_with_metadata(code, &symbols, &metadata, &mut context)?;
    let module = metadata.into_module(code, symbols)?;
    let machine = Machine {
        execution: ExecutionState::new(),
        calculator: Calculator::default(),
    };
    let mut loaded = Loadable::new(Image::new(module), machine);
    loaded.execute(&[])?;
    assert_eq!(loaded.machine().execution.stack(), &[42]);
    assert_eq!(loaded.machine().calculator.additions, 1);
    Ok(())
}
