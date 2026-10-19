// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "program-image-execution")]

use std::collections::HashMap;
use vihaco_abi::{Type, Value, traits::Reset};
use vihaco_bytecode::{NoContext, SstSectionView};
use vihaco_cpu::CPU;
use vihaco_module::{
    loader::{LoadOwnSstSection, LoadSstSection},
    module::{FunctionInfo, LocalModule, Parameter, Signature},
};
use vihaco_runtime::{
    Apply, BuildMessage, Execute, HasInstructionSet, Lower, NoMessage, Parse, Step, component,
    composite, dialect, field,
};
use vihaco_stdlib::{
    ExecutionState, Image,
    execution_state::execute,
    program::lower::{FunctionLookup, LabelLookup, StringIntern},
};

type Module<M> = LocalModule<<M as HasInstructionSet>::Runtime, Value, Type>;

#[dialect]
mod control {
    #[instruction]
    pub struct Run;
}

#[component]
#[instructions { control }]
struct Control;

impl Execute<control::Run> for Control {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    fn execute(&mut self, _: &control::Run, _: NoMessage) -> eyre::Result<()> {
        Ok(())
    }
}

#[composite]
struct ControllerMachine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    control: Control,
    #[device(2)]
    #[loadable]
    sequencer: Sequencer,
}

#[composite]
struct SequencerMachine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    cpu: vihaco_cpu::CPU,
}

struct Controller {
    image: Option<Image<<ControllerMachine as HasInstructionSet>::Runtime>>,
    machine: ControllerMachine,
}

struct Sequencer {
    image: Option<Image<<SequencerMachine as HasInstructionSet>::Runtime>>,
    machine: SequencerMachine,
}

impl LoadOwnSstSection<NoContext> for ControllerMachine {
    type Output = Module<Self>;

    fn load_own_sst_section<'src>(
        &mut self,
        section: SstSectionView<'src, NoContext>,
    ) -> eyre::Result<Self::Output> {
        lower_module::<Self>(section)
    }
}

impl LoadOwnSstSection<NoContext> for SequencerMachine {
    type Output = Module<Self>;

    fn load_own_sst_section<'src>(
        &mut self,
        section: SstSectionView<'src, NoContext>,
    ) -> eyre::Result<Self::Output> {
        lower_module::<Self>(section)
    }
}

impl LoadSstSection<NoContext> for Controller {
    type Output = ();

    fn load_sst_section<'src>(
        &mut self,
        section: SstSectionView<'src, NoContext>,
    ) -> eyre::Result<()> {
        eyre::ensure!(
            self.machine.execution.is_halted(),
            "cannot replace an active program"
        );
        let module = self.machine.load_sst_section(section)?;
        self.machine.execution.reset();
        self.image = Some(Image::new(module));
        Ok(())
    }
}

impl LoadSstSection<NoContext> for Sequencer {
    type Output = ();

    fn load_sst_section<'src>(
        &mut self,
        section: SstSectionView<'src, NoContext>,
    ) -> eyre::Result<()> {
        eyre::ensure!(
            self.machine.execution.is_halted(),
            "cannot replace an active program"
        );
        let module = self.machine.load_sst_section(section)?;
        self.machine.execution.reset();
        self.image = Some(Image::new(module));
        Ok(())
    }
}

impl Controller {
    fn run(&mut self) -> eyre::Result<()> {
        let image = self
            .image
            .as_ref()
            .ok_or_else(|| eyre::eyre!("not loaded"))?;
        execute(&image.module, &mut self.machine, &[])
    }
}

impl Sequencer {
    fn run(&mut self) -> eyre::Result<()> {
        let image = self
            .image
            .as_ref()
            .ok_or_else(|| eyre::eyre!("not loaded"))?;
        execute(&image.module, &mut self.machine, &[])
    }
}

impl<I> Step<field!(ControllerMachine::execution), I, Module<ControllerMachine>>
    for ControllerMachine
where
    ExecutionState: Execute<I, Effect = (), Error = eyre::Report>,
    Self: BuildMessage<field!(ControllerMachine::execution), I, Module<Self>>,
{
    fn step(&mut self, instruction: &I, module: &Module<Self>) -> eyre::Result<()> {
        let message = <Self as BuildMessage<
            field!(ControllerMachine::execution),
            I,
            Module<Self>,
        >>::build_message(self, instruction, module)?;
        self.execution.execute(instruction, message)
    }
}

impl Step<field!(ControllerMachine::control), control::Run, Module<ControllerMachine>>
    for ControllerMachine
{
    fn step(&mut self, instruction: &control::Run, module: &Module<Self>) -> eyre::Result<()> {
        let message = <Self as BuildMessage<
            field!(ControllerMachine::control),
            control::Run,
            Module<Self>,
        >>::build_message(self, instruction, module)?;
        self.control.execute(instruction, message)?;
        self.sequencer.run()
    }
}

impl<I> Step<field!(SequencerMachine::execution), I, Module<SequencerMachine>> for SequencerMachine
where
    ExecutionState: Execute<I, Effect = (), Error = eyre::Report>,
    Self: BuildMessage<field!(SequencerMachine::execution), I, Module<Self>>,
{
    fn step(&mut self, instruction: &I, module: &Module<Self>) -> eyre::Result<()> {
        let message = <Self as BuildMessage<
            field!(SequencerMachine::execution),
            I,
            Module<Self>,
        >>::build_message(self, instruction, module)?;
        self.execution.execute(instruction, message)
    }
}

impl<I> Step<field!(SequencerMachine::cpu), I, Module<SequencerMachine>> for SequencerMachine
where
    CPU: Execute<I, Error = eyre::Report>,
    Self: BuildMessage<field!(SequencerMachine::cpu), I, Module<Self>>,
    ExecutionState: Apply<<CPU as Execute<I>>::Effect>,
{
    fn step(&mut self, instruction: &I, module: &Module<Self>) -> eyre::Result<()> {
        let message =
            <Self as BuildMessage<field!(SequencerMachine::cpu), I, Module<Self>>>::build_message(
                self,
                instruction,
                module,
            )?;
        let effect = self.cpu.execute(instruction, message)?;
        self.execution.apply(effect);
        Ok(())
    }
}

// Section-local loader capabilities, not a second execution engine.
struct Symbols<I> {
    module: LocalModule<I, Value, Type>,
    functions: HashMap<String, (u32, u32)>,
    labels: HashMap<String, u32>,
}

impl<I> FunctionLookup for Symbols<I> {
    fn function_address(&self, name: &str) -> eyre::Result<u32> {
        self.functions
            .get(name)
            .map(|&(_, address)| address)
            .ok_or_else(|| eyre::eyre!("unknown function {name}"))
    }
    fn function_index(&self, name: &str) -> eyre::Result<u32> {
        self.functions
            .get(name)
            .map(|&(index, _)| index)
            .ok_or_else(|| eyre::eyre!("unknown function {name}"))
    }
}

impl<I> LabelLookup for Symbols<I> {
    fn label_address(&self, name: &str) -> eyre::Result<u32> {
        self.labels
            .get(name)
            .copied()
            .ok_or_else(|| eyre::eyre!("unknown label {name}"))
    }
}

impl<I> StringIntern for Symbols<I> {
    fn intern_string(&mut self, value: &str) -> eyre::Result<u32> {
        self.module.intern_string(value)
    }
}

struct Header;

impl vihaco_abi::traits::FromText for Header {
    fn from_text(_: &str) -> eyre::Result<Self> {
        Ok(Self)
    }
}

impl vihaco_bytecode::SstHeader for Header {}

fn lower_module<'src, M>(section: SstSectionView<'src, NoContext>) -> eyre::Result<Module<M>>
where
    M: HasInstructionSet,
    M::Syntax: Parse<'src> + 'src + Lower<Symbols<M::Runtime>, Runtime = M::Runtime>,
{
    let parsed = vihaco_syntax::ParsedModule::<M::Syntax, Type, Header>::parse_section(section)?;
    let mut loader = Symbols {
        module: LocalModule::default(),
        functions: HashMap::new(),
        labels: HashMap::new(),
    };
    let mut address = 0u32;
    for (index, function) in parsed.functions.iter().enumerate() {
        eyre::ensure!(
            loader
                .functions
                .insert(
                    function.name.as_str().to_owned(),
                    (u32::try_from(index)?, address)
                )
                .is_none(),
            "duplicate function"
        );
        address = address
            .checked_add(u32::try_from(function.body.len())?)
            .ok_or_else(|| eyre::eyre!("program too large"))?;
    }
    loader.module.main_function = Some(loader.function_index("main")?);
    for function in parsed.functions {
        let start = u32::try_from(loader.module.code.len())?;
        loader.labels.clear();
        for label in function.labels {
            let address = start
                .checked_add(u32::try_from(label.instruction_index)?)
                .ok_or_else(|| eyre::eyre!("label overflow"))?;
            eyre::ensure!(
                loader
                    .labels
                    .insert(label.name.as_str().to_owned(), address)
                    .is_none(),
                "duplicate label"
            );
        }
        let name = loader.intern_string(function.name.as_str())?;
        let params = function
            .params
            .into_iter()
            .map(|p| {
                Ok(Parameter {
                    name: loader.intern_string(p.name.as_str())?,
                    ty: p.ty,
                })
            })
            .collect::<eyre::Result<Vec<_>>>()?;
        let local_count = u32::try_from(params.len())?;
        for instruction in function.body {
            let instruction = instruction.lower(&mut loader)?;
            loader.module.code.push(instruction);
        }
        loader.module.functions.push(FunctionInfo {
            name,
            signature: Signature {
                params,
                ret: function.return_ty.into_iter().collect(),
            },
            local_count,
            start_address: start,
            end_address: u32::try_from(loader.module.code.len())?,
            file: 0,
        });
    }
    Ok(loader.module)
}

#[test]
fn nested_owners_load_lower_and_execute_independent_programs() {
    let mut controller = Controller {
        image: None,
        machine: ControllerMachine {
            execution: ExecutionState::new(),
            control: Control,
            sequencer: Sequencer {
                image: None,
                machine: SequencerMachine {
                    execution: ExecutionState::new(),
                    cpu: CPU::default(),
                },
            },
        },
    };
    assert!(controller.run().is_err());
    let source = format!(
        r#"sst v{}
.section(root):
.text(root):
fn @main() {{
    control::control::run
    execution::program::call 0, @helper
    execution::program::ret 1
}}
fn @helper() {{
    execution::program::const_u64 11
    execution::program::ret 1
}}
.text(root).
.section(sequencer):
.text(sequencer):
fn @main() {{
    execution::program::call 0, @helper
    execution::program::const_u64 2
    cpu::cpu::mul_u64
    execution::program::ret 1
}}
fn @helper() {{
    execution::program::const_u64 21
    execution::program::ret 1
}}
.text(sequencer).
.section(sequencer).
.section(root).
"#,
        vihaco_bytecode::VERSION
    );
    let file = vihaco_bytecode::SstFile::<NoContext>::from_text(&source).unwrap();
    controller.load_sst_section(file.root()).unwrap();
    // Loaded artifacts own their code and metadata independently of the source file.
    drop(file);
    drop(source);
    controller.run().unwrap();
    assert_eq!(controller.machine.execution.stack(), &[11]);
    assert_eq!(
        controller.machine.sequencer.machine.execution.stack(),
        &[42]
    );
}
