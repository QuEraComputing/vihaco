// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "program-image-execution")]

use vihaco_abi::{Type, Value, traits::FromText};
use vihaco_bytecode::{NoContext, SstFile, SstHeader, SstSectionView};
use vihaco_cpu::CPU;
use vihaco_module::{
    loader::MetadataLoweringContext,
    module::{FunctionInfo, LocalModule, ProgramMetadata, Signature},
};
use vihaco_runtime::{
    Apply, BuildMessage, Composite, Dispatch, Execute, HasInstructionSet, HasProgram, Lower,
    NoMessage, Parse, Step, chumsky::prelude::*, composite, field,
};
use vihaco_stdlib::{ExecutionState, Image, Loadable, program::program};

type Module<M> = LocalModule<<M as HasInstructionSet>::Runtime, Value, Type>;
type MachineImage<M> = Image<<M as HasInstructionSet>::Runtime>;

#[composite]
struct ControllerMachine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    visits: usize,
    fail_once: bool,
}

#[composite]
struct SequencerMachine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    cpu: vihaco_cpu::CPU,
}

impl<I> Step<field!(ControllerMachine::execution), I, Module<ControllerMachine>>
    for ControllerMachine
where
    ExecutionState: Execute<I, Effect = (), Error = eyre::Report>,
    Self: BuildMessage<field!(ControllerMachine::execution), I, Module<Self>>,
{
    fn step(&mut self, instruction: &I, module: &Module<Self>) -> eyre::Result<()> {
        self.visits += 1;
        if std::mem::take(&mut self.fail_once) {
            eyre::bail!("injected step failure");
        }
        let message = <Self as BuildMessage<
            field!(ControllerMachine::execution),
            I,
            Module<Self>,
        >>::build_message(self, instruction, module)?;
        self.execution.execute(instruction, message)
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

struct Header(ProgramMetadata);

impl FromText for Header {
    fn from_text(text: &str) -> eyre::Result<Self> {
        ProgramMetadata::parser()
            .padded_by(vihaco_syntax::skip())
            .then_ignore(end())
            .parse(text)
            .into_result()
            .map(Self)
            .map_err(|errors| eyre::eyre!("invalid metadata: {errors:?}"))
    }
}

impl SstHeader for Header {}

fn construct_image<'src, M>(
    section: SstSectionView<'src, NoContext>,
) -> eyre::Result<MachineImage<M>>
where
    M: HasInstructionSet,
    M::Syntax: Parse<'src> + 'src + Lower<MetadataLoweringContext, Runtime = M::Runtime>,
{
    let vihaco_syntax::ParsedModule { header, functions } =
        vihaco_syntax::ParsedModule::<M::Syntax, Type, Header>::parse_section(section)?;
    let metadata = header.0;
    let mut context = MetadataLoweringContext::new(&metadata)?;
    let module = vihaco_syntax::ParsedModule {
        header: (),
        functions,
    }
    .lower_with_metadata(metadata, &mut context)?;
    Ok(Image::new(module))
}

fn section_source(name: &str, main: &str, helper: &str) -> String {
    let helper_start = main.lines().count();
    let end = helper_start + helper.lines().count();
    // Table order intentionally differs from executable function order.
    format!(
        r#".section({name}):
.header({name}):
metadata {{
    strings: ["main", "helper", "source.sst"],
    functions: [
        {{ name: 1, signature: {{ params: [], ret: [u64] }}, local_count: 0,
           start_address: {helper_start}, end_address: {end}, file: 2 }},
        {{ name: 0, signature: {{ params: [], ret: [u64] }}, local_count: 0,
           start_address: 0, end_address: {helper_start}, file: 2 }},
    ],
    labels: [], constants: [], main_function: 1, source_symbols: [], extra: {{}},
}}
.header({name}).
.text({name}):
fn @main() -> u64 {{
{main}
}}
fn @helper() -> u64 {{
{helper}
}}
.text({name}).
.section({name}).
"#
    )
}

fn controller_image(value: u64) -> MachineImage<ControllerMachine> {
    let source = format!(
        "sst v{}\n{}",
        vihaco_bytecode::VERSION,
        section_source(
            "root",
            "execution::program::call 0, @helper\nexecution::program::ret 1",
            &format!("execution::program::const_u64 {value}\nexecution::program::ret 1"),
        )
    );
    let file = SstFile::<NoContext>::from_text(&source).unwrap();
    construct_image::<ControllerMachine>(file.root()).unwrap()
}

fn controller() -> Loadable<MachineImage<ControllerMachine>, ControllerMachine> {
    Loadable::new(
        controller_image(11),
        ControllerMachine {
            execution: ExecutionState::new(),
            visits: 0,
            fail_once: false,
        },
    )
}

#[test]
fn sibling_loadables_construct_and_execute_independent_programs() {
    struct Runtime {
        controller: Loadable<MachineImage<ControllerMachine>, ControllerMachine>,
        sequencer: Loadable<MachineImage<SequencerMachine>, SequencerMachine>,
    }
    let child = section_source(
        "sequencer",
        "execution::program::call 0, @helper\nexecution::program::const_u64 2\ncpu::cpu::mul_u64\nexecution::program::ret 1",
        "execution::program::const_u64 21\nexecution::program::ret 1",
    );
    let root = section_source(
        "root",
        "execution::program::call 0, @helper\nexecution::program::ret 1",
        "execution::program::const_u64 11\nexecution::program::ret 1",
    );
    let source = format!(
        "sst v{}\n{}",
        vihaco_bytecode::VERSION,
        root.replace(".section(root).", &format!("{child}.section(root).")),
    );
    let file = SstFile::<NoContext>::from_text(&source).unwrap();
    let controller_program = construct_image::<ControllerMachine>(file.root()).unwrap();
    let sequencer_image =
        construct_image::<SequencerMachine>(file.root().child("sequencer").unwrap()).unwrap();
    let mut runtime = Runtime {
        controller: Loadable::new(
            controller_program,
            ControllerMachine {
                execution: ExecutionState::new(),
                visits: 0,
                fail_once: false,
            },
        ),
        sequencer: Loadable::new(
            sequencer_image,
            SequencerMachine {
                execution: ExecutionState::new(),
                cpu: CPU::default(),
            },
        ),
    };
    drop(file);
    drop(source);
    assert_eq!(runtime.controller.image().module.main_function, Some(1));
    runtime.controller.execute(&[]).unwrap();
    runtime.sequencer.execute(&[]).unwrap();
    assert_eq!(runtime.controller.machine().execution.stack(), &[11]);
    assert_eq!(runtime.sequencer.machine().execution.stack(), &[42]);
    runtime
        .controller
        .replace_image(controller_image(17))
        .unwrap();
    runtime.controller.execute(&[]).unwrap();
    assert_eq!(runtime.controller.machine().execution.stack(), &[17]);
    assert_eq!(runtime.sequencer.machine().execution.stack(), &[42]);
}

#[test]
fn construction_resets_previous_invocation_without_resetting_machine_state() {
    let image = controller_image(11);
    let mut machine = ControllerMachine {
        execution: ExecutionState::new(),
        visits: 7,
        fail_once: false,
    };
    machine.execution.start(&image.module, &[]).unwrap();
    assert!(!machine.execution.is_halted());
    let loaded = Loadable::new(image, machine);
    assert!(loaded.machine().execution.is_halted());
    assert!(loaded.machine().execution.frames().is_empty());
    assert_eq!(loaded.machine().visits, 7);
}

#[test]
fn active_replacement_is_rejected_and_faulted_invocation_can_resume() {
    let mut loaded = controller();
    loaded.machine_mut().fail_once = true;
    assert!(
        loaded
            .execute(&[])
            .unwrap_err()
            .to_string()
            .contains("injected")
    );
    let image = loaded.image().module.code.as_ptr();
    let pc = loaded.machine().execution.pc();
    let frames = loaded.machine().execution.frames().len();
    let visits = loaded.machine().visits;
    assert!(loaded.replace_image(controller_image(17)).is_err());
    assert_eq!(loaded.image().module.code.as_ptr(), image);
    assert_eq!(loaded.machine().execution.pc(), pc);
    assert_eq!(loaded.machine().execution.frames().len(), frames);
    assert_eq!(loaded.machine().visits, visits);
    loaded.resume().unwrap();
    assert_eq!(loaded.machine().execution.stack(), &[11]);

    loaded.replace_image(controller_image(17)).unwrap();
    assert!(loaded.machine().execution.stack().is_empty());
    assert!(loaded.machine().execution.frames().is_empty());
    assert_eq!(loaded.machine().execution.pc(), 0);
    assert!(loaded.machine().visits > visits);
    loaded.execute(&[]).unwrap();
    assert_eq!(loaded.machine().execution.stack(), &[17]);
}

#[test]
fn explicit_reset_abandons_invocation_and_extraction_preserves_parts() {
    let mut loaded = controller();
    loaded.machine_mut().fail_once = true;
    assert!(loaded.execute(&[]).is_err());
    loaded.reset();
    assert!(loaded.machine().execution.is_halted());
    assert!(loaded.machine().execution.frames().is_empty());
    assert_eq!(loaded.machine().visits, 1);
    loaded.replace_image(controller_image(17)).unwrap();
    loaded.execute(&[]).unwrap();
    let image_pointer = loaded.image().module.code.as_ptr();
    let visits = loaded.machine().visits;
    let (image, machine) = loaded.into_parts();
    assert_eq!(image.module.code.as_ptr(), image_pointer);
    assert_eq!(machine.visits, visits);
    assert_eq!(machine.execution.stack(), &[17]);
}

#[test]
fn failed_image_construction_leaves_installed_program_unchanged() {
    let mut loaded = controller();
    let source = format!(
        "sst v{}\n{}",
        vihaco_bytecode::VERSION,
        section_source(
            "root",
            "execution::program::call 0, @missing\nexecution::program::ret 1",
            "execution::program::const_u64 17\nexecution::program::ret 1",
        )
    );
    let file = SstFile::<NoContext>::from_text(&source).unwrap();
    let image_pointer = loaded.image().module.code.as_ptr();
    assert!(construct_image::<ControllerMachine>(file.root()).is_err());
    assert_eq!(loaded.image().module.code.as_ptr(), image_pointer);
    assert_eq!(loaded.machine().visits, 0);
    loaded.execute(&[]).unwrap();
    assert_eq!(loaded.machine().execution.stack(), &[11]);
}

#[test]
fn execution_borrows_non_clone_instructions_from_the_paired_image() {
    struct Instruction(Box<u64>);
    type Program = LocalModule<Instruction, Value, Type>;
    struct Machine {
        execution: ExecutionState,
        observed: u64,
    }
    impl Composite for Machine {}
    impl HasProgram for Machine {
        type Program = ExecutionState;
        fn program(&self) -> &ExecutionState {
            &self.execution
        }
        fn program_mut(&mut self) -> &mut ExecutionState {
            &mut self.execution
        }
    }
    impl Dispatch<Program> for Machine {
        type Instruction = Instruction;
        fn dispatch(&mut self, instruction: &Instruction, module: &Program) -> eyre::Result<()> {
            assert!(std::ptr::eq(instruction, &module.code[0]));
            self.observed = *instruction.0;
            self.execution.execute(&program::Halt, NoMessage::new())
        }
    }
    let image = Image::new(Program {
        code: vec![Instruction(Box::new(42))],
        main_function: Some(0),
        functions: vec![FunctionInfo {
            name: 0,
            signature: Signature {
                params: vec![],
                ret: vec![],
            },
            local_count: 0,
            start_address: 0,
            end_address: 1,
            file: 0,
        }],
        ..Default::default()
    });
    let mut loaded = Loadable::new(
        image,
        Machine {
            execution: ExecutionState::new(),
            observed: 0,
        },
    );
    loaded.execute(&[]).unwrap();
    assert_eq!(loaded.machine().observed, 42);
}
