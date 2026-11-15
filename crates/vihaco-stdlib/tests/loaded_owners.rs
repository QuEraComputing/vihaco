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
use vihaco_stdlib::{ExecutionState, Image, Loadable, Verified, program::program};
use vihaco_verifier::{ModuleVerifier, Verify};

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

fn construct_module<'src, M>(section: SstSectionView<'src, NoContext>) -> eyre::Result<Module<M>>
where
    M: HasInstructionSet,
    M::Syntax: Parse<'src> + 'src + Lower<MetadataLoweringContext, Runtime = M::Runtime>,
{
    let vihaco_syntax::ParsedModule { header, functions } =
        vihaco_syntax::ParsedModule::<M::Syntax, Type, Header>::parse_section(section)?;
    let metadata = header.0;
    let mut context = MetadataLoweringContext::new(&metadata)?;
    vihaco_syntax::ParsedModule {
        header: (),
        functions,
    }
    .lower_with_metadata(metadata, &mut context)
}

/// Parse, lower, and verify a section into an image.
fn construct_image<'src, M>(
    section: SstSectionView<'src, NoContext>,
) -> eyre::Result<MachineImage<M>>
where
    M: HasInstructionSet,
    M::Syntax: Parse<'src> + 'src + Lower<MetadataLoweringContext, Runtime = M::Runtime>,
    // A rejection displays with the module's names, so its instructions show.
    M::Runtime: for<'m> Verify<ModuleVerifier<'m>>
        + vihaco_runtime::InstructionText
        + Send
        + Sync
        + 'static,
{
    Ok(Image::new(Verified::new(construct_module::<M>(section)?)?))
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
        {{ name: 1, signature: {{ params: [], ret: [u64] }}, locals: [],
           start_address: {helper_start}, end_address: {end}, file: 2 }},
        {{ name: 0, signature: {{ params: [], ret: [u64] }}, locals: [],
           start_address: 0, end_address: {helper_start}, file: 2 }},
    ],
    structs: [], types: [], labels: [], constants: [], main_function: 1, source_symbols: [],
    extra: {{}},
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
    assert_eq!(runtime.controller.image().module().main_function, Some(1));
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

fn struct_program(main_body: &str) -> String {
    // main: 0..n; make_point: 4 instructions after it.
    let main_end = main_body.lines().count();
    let point_end = main_end + 4;
    format!(
        r#"sst v{version}
.section(root):
.header(root):
metadata {{
    strings: ["main", "make_point", "points.sst", "Point", "x", "y", "location"],
    structs: [
        {{ name: 3, fields: [{{ name: 4, ty: u64 }}, {{ name: 5, ty: u64 }}] }},
    ],
    types: [{{ name: 6, repr: u64 }}],
    functions: [
        {{ name: 0, signature: {{ params: [], ret: [u64] }}, locals: [],
           start_address: 0, end_address: {main_end}, file: 2 }},
        {{ name: 1, signature: {{ params: [], ret: [struct 0] }}, locals: [],
           start_address: {main_end}, end_address: {point_end}, file: 2 }},
    ],
    labels: [], constants: [], main_function: 0, source_symbols: [], extra: {{}},
}}
.header(root).
.text(root):
fn @main() -> u64 {{
{main_body}
}}
fn @make_point() -> struct 0 {{
execution::program::const_u64 7
execution::program::const_u64 9
cpu::cpu::struct_new @Point
execution::program::ret 1
}}
.text(root).
.section(root).
"#,
        version = vihaco_bytecode::VERSION,
    )
}

/// Parse, lower, and verify a struct program: well-formed fixtures pass
/// load-time verification against the composite's generated rules.
fn load_struct_program(main_body: &str) -> eyre::Result<MachineImage<SequencerMachine>> {
    let source = struct_program(main_body);
    let file = SstFile::<NoContext>::from_text(&source)?;
    construct_image::<SequencerMachine>(file.root())
}

/// Parse and lower a struct program without verifying it.
fn load_unverified_struct_program(main_body: &str) -> eyre::Result<Module<SequencerMachine>> {
    let source = struct_program(main_body);
    let file = SstFile::<NoContext>::from_text(&source)?;
    construct_module::<SequencerMachine>(file.root())
}

#[test]
fn struct_fields_resolve_by_name_through_the_metadata_struct_table() {
    // let p = make_point(); p.y = 4; return p.y
    let image = load_struct_program(
        "execution::program::call 0, @make_point
execution::program::dup
execution::program::const_u64 4
cpu::cpu::set_field @Point.y
cpu::cpu::get_field @Point.y
execution::program::ret 1",
    )
    .unwrap();
    assert_eq!(image.module().structs.len(), 1);
    assert_eq!(image.module().functions[1].signature.ret, [Type::Struct(0)]);
    let mut loaded = Loadable::new(
        image,
        SequencerMachine {
            execution: ExecutionState::new(),
            cpu: CPU::default(),
        },
    );
    loaded.execute(&[]).unwrap();
    assert_eq!(loaded.machine().execution.stack(), &[4]);
}

fn run_struct_program(main_body: &str) -> eyre::Result<Vec<u64>> {
    run_image(load_struct_program(main_body)?)
}

fn run_image(image: MachineImage<SequencerMachine>) -> eyre::Result<Vec<u64>> {
    let mut loaded = Loadable::new(
        image,
        SequencerMachine {
            execution: ExecutionState::new(),
            cpu: CPU::default(),
        },
    );
    loaded.execute(&[])?;
    Ok(loaded.machine().execution.stack().to_vec())
}

#[test]
fn arrays_of_named_structs_are_built_indexed_and_type_checked() {
    // let points = [make_point()]; return points[0].y
    let source = "execution::program::call 0, @make_point
cpu::cpu::array_new struct @Point, 1
execution::program::const_u64 0
cpu::cpu::array_get struct @Point
cpu::cpu::get_field @Point.y
execution::program::ret 1";
    let stack = run_struct_program(source).unwrap();
    assert_eq!(stack, [9]);

    // Reading the same array as `array u64` is a runtime type error, and the
    // verifier rejects the program before it runs.
    let ill_typed = "execution::program::call 0, @make_point
cpu::cpu::array_new struct @Point, 1
execution::program::const_u64 0
cpu::cpu::array_get u64
cpu::cpu::get_field @Point.y
execution::program::ret 1";
    let module = load_unverified_struct_program(ill_typed).unwrap();
    let rejection =
        vihaco_verifier::verify_module(&module, vihaco_verifier::VerifyOptions::default())
            .unwrap_err();
    let rendered = rejection.diagnostics[0].display_with(&module).to_string();
    assert_eq!(
        rendered,
        "error: operand 0 of `cpu::cpu::array_get`: expected array u64, found array struct \
         Point\n  --> @main+3 (address 3): cpu::cpu::array_get u64"
    );
    // Constructing the image verifies it, so only the escape hatch runs it.
    let error = load_struct_program(ill_typed).unwrap_err();
    assert!(
        format!("{error:#}").contains("verification failed"),
        "{error:#}"
    );
    // At runtime the heap check fails, located and with types by name.
    let error = run_image(Image::new(Verified::assume_verified(module))).unwrap_err();
    assert_eq!(
        error.to_string(),
        "at @main+3 (address 3) `cpu::cpu::array_get u64`: type error: heap object (slot 2, \
         generation 1) is array struct Point, expected array u64"
    );
    let step = error
        .downcast_ref::<vihaco_stdlib::execution_state::StepError>()
        .unwrap();
    assert_eq!(step.address(), 3);
    assert!(
        step.error()
            .downcast_ref::<vihaco_abi::heap::HeapError>()
            .is_some()
    );
    // The step's error is the StepError's source, so the report's chain
    // reaches the HeapError without knowing about StepError.
    let heap = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<vihaco_abi::heap::HeapError>());
    assert!(
        matches!(heap, Some(vihaco_abi::heap::HeapError::TypeMismatch { .. })),
        "{heap:?}"
    );
    assert_eq!(error.chain().count(), 2);

    // An element type naming an undeclared struct fails at load time.
    let error = load_struct_program(
        "execution::program::call 0, @make_point
cpu::cpu::array_new struct @Line, 1
execution::program::const_u64 0
cpu::cpu::array_get struct @Point
cpu::cpu::get_field @Point.y
execution::program::ret 1",
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("lowering @main+1 (address 1): undefined struct `@Line`"),
        "{error:#}"
    );
}

#[test]
fn named_types_resolve_by_name_through_the_metadata_type_table() {
    // let locations = [wrap(7)]; return unwrap(locations[0])
    let source = "execution::program::const_u64 7
cpu::cpu::wrap @location
cpu::cpu::array_new type @location, 1
execution::program::const_u64 0
cpu::cpu::array_get type @location
cpu::cpu::unwrap @location
execution::program::ret 1";
    let image = load_struct_program(source).unwrap();
    assert_eq!(image.module().types.len(), 1);
    let code: Vec<String> = image
        .module()
        .code
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        code.contains(&"cpu::cpu::array_new type 0, 1".into()),
        "{code:?}"
    );
    assert!(code.contains(&"cpu::cpu::wrap 0".into()), "{code:?}");
    assert_eq!(run_struct_program(source).unwrap(), [7]);

    // Numeric references are checked against the table too.
    let stack = run_struct_program(
        "execution::program::const_u64 7
cpu::cpu::wrap 0
cpu::cpu::unwrap @location
execution::program::ret 1",
    )
    .unwrap();
    assert_eq!(stack, [7]);
    for (body, message) in [
        ("cpu::cpu::wrap @lane", "undefined named type `@lane`"),
        ("cpu::cpu::wrap 1", "outside the named-type table"),
        (
            "cpu::cpu::array_new type @lane, 1",
            "undefined named type `@lane`",
        ),
        (
            "cpu::cpu::array_get array type 3",
            "outside the named-type table",
        ),
    ] {
        let error = load_struct_program(&format!(
            "execution::program::const_u64 7
{body}
execution::program::ret 1"
        ))
        .unwrap_err();
        assert!(format!("{error:#}").contains(message), "{body}: {error:#}");
    }
}

#[test]
fn unknown_struct_fields_fail_at_load_time() {
    let error = load_struct_program(
        "execution::program::call 0, @make_point
execution::program::dup
execution::program::const_u64 4
cpu::cpu::set_field @Point.z
cpu::cpu::get_field @Point.y
execution::program::ret 1",
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("struct `Point` has no field `z`"),
        "{error:#}"
    );
}

#[test]
fn construction_resets_previous_invocation_without_resetting_machine_state() {
    let image = controller_image(11);
    let mut machine = ControllerMachine {
        execution: ExecutionState::new(),
        visits: 7,
        fail_once: false,
    };
    machine.execution.start(image.module(), &[]).unwrap();
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
    let image = loaded.image().module().code.as_ptr();
    let pc = loaded.machine().execution.pc();
    let frames = loaded.machine().execution.frames().len();
    let visits = loaded.machine().visits;
    assert!(loaded.replace_image(controller_image(17)).is_err());
    assert_eq!(loaded.image().module().code.as_ptr(), image);
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
    let image_pointer = loaded.image().module().code.as_ptr();
    let visits = loaded.machine().visits;
    let (image, machine) = loaded.into_parts();
    assert_eq!(image.module().code.as_ptr(), image_pointer);
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
    let image_pointer = loaded.image().module().code.as_ptr();
    assert!(construct_image::<ControllerMachine>(file.root()).is_err());
    assert_eq!(loaded.image().module().code.as_ptr(), image_pointer);
    assert_eq!(loaded.machine().visits, 0);
    loaded.execute(&[]).unwrap();
    assert_eq!(loaded.machine().execution.stack(), &[11]);
}

#[test]
fn execution_borrows_non_clone_instructions_from_the_paired_image() {
    // A hand-written instruction set needs neither `Clone` nor `Display`:
    // without `Dispatch::instruction_text`, errors show the location alone.
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
    // A test-local instruction set with no verifier rules.
    let image = Image::new(Verified::assume_verified(Program {
        code: vec![Instruction(Box::new(42))],
        main_function: Some(0),
        functions: vec![FunctionInfo {
            name: 0,
            signature: Signature {
                params: vec![],
                ret: vec![],
            },
            locals: vec![],
            start_address: 0,
            end_address: 1,
            file: 0,
        }],
        ..Default::default()
    }));
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
