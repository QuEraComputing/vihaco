// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Run with:
//! `cargo run -p vihaco-stdlib --features program-image-execution --example loadable`
//!
//! Pass an SST file path as the final argument to load another source file.
//! Without a path, this runs the adjacent `loadable.sst`, which computes 21 * 2.

use vihaco_abi::{Type, Value};
use vihaco_bytecode::{NoContext, SSTFile};
use vihaco_cpu::CPU;
use vihaco_module::{
    loader::MetadataLoweringContext,
    module::{LocalModule, ProgramMetadata},
};
use vihaco_runtime::{Apply, BuildMessage, Execute, HasInstructionSet, Step, composite, field};
use vihaco_stdlib::{ExecutionState, Image, Loadable};
use vihaco_syntax::{HeaderDeclarations, ParsedModule, lower_code_with_metadata};

// This composite owns mutable devices and invocation state, while its image
// will live in the separate Loadable field below.
#[composite]
struct Machine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    cpu: vihaco_cpu::CPU,
}

type Instruction = <Machine as HasInstructionSet>::Runtime;
type SourceInstruction = <Machine as HasInstructionSet>::Syntax;
type Program = LocalModule<Instruction, Value, Type, ExperimentInfo>;
type ControllerImage = Image<Instruction, Value, Type, ExperimentInfo>;

// Each Step supplies runtime input and chooses how to handle execution effects.
// Control/storage instructions mutate ExecutionState directly.
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

// Computation instructions consume operands through BuildMessage and return
// effects that this policy applies to the invocation's operand stack.
impl<I> Step<field!(Machine::cpu), I, Program> for Machine
where
    CPU: Execute<I, Error = eyre::Report>,
    Self: BuildMessage<field!(Machine::cpu), I, Program>,
    ExecutionState: Apply<<CPU as Execute<I>>::Effect>,
{
    fn step(&mut self, instruction: &I, module: &Program) -> eyre::Result<()> {
        let message = <Self as BuildMessage<field!(Machine::cpu), I, Program>>::build_message(
            self,
            instruction,
            module,
        )?;
        let effect = self.cpu.execute(instruction, message)?;
        self.execution.apply(effect);
        Ok(())
    }
}

#[derive(Debug)]
struct ExperimentInfo {
    description: String,
}

type Metadata = ProgramMetadata<Value, Type>;

// A header is a collection of declarations. Each entry is collected separately;
// metadata can appear before or after other entries.
#[derive(Debug, vihaco_parser_derive::Parse)]
#[syntax_class(metadata)]
enum HeaderDeclaration {
    #[pattern = "$0"]
    Metadata(Metadata),
    #[pattern = "experiment.description $0"]
    Description(String),
}

#[derive(Default)]
struct HeaderDeclarationsState {
    metadata: Option<Metadata>,
    description: Option<String>,
}

impl HeaderDeclaration {
    fn collect_into(self, declarations: &mut HeaderDeclarationsState) -> eyre::Result<()> {
        match self {
            Self::Metadata(metadata) => {
                eyre::ensure!(
                    declarations.metadata.is_none(),
                    "duplicate metadata declaration"
                );
                declarations.metadata = Some(metadata);
            }
            Self::Description(description) => {
                eyre::ensure!(
                    declarations.description.is_none(),
                    "duplicate description declaration"
                );
                eyre::ensure!(
                    !description.is_empty(),
                    "experiment.description must not be empty"
                );
                declarations.description = Some(description);
            }
        }
        Ok(())
    }
}

impl HeaderDeclarationsState {
    fn finish_program(self) -> eyre::Result<(Metadata, ExperimentInfo)> {
        let metadata = self
            .metadata
            .ok_or_else(|| eyre::eyre!("missing program metadata"))?;
        let description = self
            .description
            .ok_or_else(|| eyre::eyre!("missing experiment.description"))?;
        Ok((metadata, ExperimentInfo { description }))
    }
}

// An ordinary Rust runtime holds independently loaded programs. It has no
// composite or loadable attributes and selects file sections explicitly.
struct Runtime {
    controller: Loadable<ControllerImage, Machine>,
}

impl Runtime {
    fn from_sst(source: &str) -> eyre::Result<Self> {
        // 1. Read the container and select this runtime field's program section.
        let file = SSTFile::<NoContext>::from_text(source)?;
        let section = file
            .section("controller")
            .ok_or_else(|| eyre::eyre!("missing controller section"))?;

        // 2. Parse header declarations and the composite's syntax instructions.
        let ParsedModule {
        header,
        code,
        symbols,
    } = ParsedModule::<
            SourceInstruction,
            HeaderDeclarations<HeaderDeclaration>,
        >::parse_section(section)?;

        // 3. Collect declarations and reject duplicates, then check the required
        // metadata and description for this program.
        let mut header_state = HeaderDeclarationsState::default();
        for declaration in header {
            declaration.collect_into(&mut header_state)?;
        }
        let (metadata, info) = header_state.finish_program()?;

        // 4. Build symbol capabilities from the metadata and the section's symbol
        // table, validate declarations/tables, and lower the code stream.
        let mut context = MetadataLoweringContext::new(&metadata, &symbols)?;
        let code = lower_code_with_metadata(code, &symbols, &metadata, &mut context)?;
        let module = metadata.map_extra(|_| info).into_module(code, symbols)?;

        // 5. Wrap the completed module and pair it with a fresh execution machine.
        let image = Image::new(module);
        let machine = Machine {
            execution: ExecutionState::new(),
            cpu: CPU::default(),
        };
        Ok(Self {
            controller: Loadable::new(image, machine),
        })
        // The container and lowering context are dropped here. The image owns
        // its runtime instructions and metadata, independently of the source.
    }
}

fn main() -> eyre::Result<()> {
    let source = match std::env::args_os().nth(1) {
        Some(path) => std::fs::read_to_string(path)?,
        None => include_str!("loadable.sst").to_owned(),
    };
    let mut runtime = Runtime::from_sst(&source)?;
    drop(source);

    println!("{}", runtime.controller.image().module.extra.description);
    println!(
        "Loaded {} instructions",
        runtime.controller.image().module.code.len()
    );

    // 6. Enter main. Dispatch borrows instructions and metadata from the image
    // while the authored Steps mutate the separate machine.
    runtime.controller.execute(&[])?;
    println!(
        "Results: {:?}",
        runtime.controller.machine().execution.stack()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collected_declarations_and_lowered_bodies_form_an_executable_image() -> eyre::Result<()> {
        let mut runtime = Runtime::from_sst(include_str!("loadable.sst"))?;
        assert_eq!(runtime.controller.image().module.main_function, Some(1));
        assert_eq!(
            runtime.controller.image().module.extra.description,
            "Loadable SST example: 21 * 2"
        );
        runtime.controller.execute(&[])?;
        assert_eq!(runtime.controller.machine().execution.stack(), &[42]);
        Ok(())
    }

    impl vihaco_sst::check::MetadataDeclaration for HeaderDeclaration {
        type Value = Value;
        type Type = Type;
        type Info = vihaco_module::module::NoInfo;

        fn as_metadata(&self) -> Option<&Metadata> {
            match self {
                Self::Metadata(metadata) => Some(metadata),
                Self::Description(_) => None,
            }
        }
    }

    fn check(source: &str) -> Vec<String> {
        let source = vihaco_sst::SourceFile::new("loadable.sst", source);
        vihaco_sst::ProgramChecker::<Machine>::new()
            .with_header::<vihaco_sst::Declarations<HeaderDeclaration>>()
            .require_sections(["controller"])
            .check(&source)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// `loadable.sst` in the formatter's canonical layout.
    const FORMATTED: &str = r#"sst v2

.section controller
.header
    experiment.description "Loadable SST example: 21 * 2";
    metadata {
        // Table order is independent of executable function order.
        functions: [
            {
                name: 1,   // strings[1]: helper //! "helper"
                signature: { params: [], ret: [u64] },
                local_count: 0,
                file: 2,   //! "loadable.sst"
            },
            {
                name: 0,   // strings[0]: main //! "main"
                signature: { params: [], ret: [u64] },
                local_count: 0,
                file: 2,   //! "loadable.sst"
            },
        ],
        constants: [],
        strings: ["main", "helper", "loadable.sst"],
        main_function: 1,  // functions[1], not instruction address 1. //! @main
        source_symbols: [],
        extra: {},
    };
.text
@main:                                   //! fn main() -> u64, locals 0, functions[1]
    execution::program::call 0, @helper  //! @helper = 0x0006 (functions[0])
    execution::program::const_u64 2
    cpu::cpu::mul_u64
    execution::program::br @done         //! @done = 0x0005
    execution::program::const_u64 999
@done:                                   //! 0x0005
    execution::program::ret 1
@helper:                                 //! fn helper() -> u64, locals 0, functions[0]
    execution::program::const_u64 21
    execution::program::ret 1
"#;

    fn format(text: &str, options: vihaco_sst::FormatOptions) -> String {
        let source = vihaco_sst::SourceFile::new("loadable.sst", text);
        let checker = vihaco_sst::ProgramChecker::<Machine>::new()
            .with_header::<vihaco_sst::Declarations<HeaderDeclaration>>();
        vihaco_sst::Formatter::with_checker(checker)
            .options(options)
            .format(&source)
            .unwrap_or_else(|report| panic!("{}", report.render(&source)))
    }

    #[test]
    fn the_formatted_example_is_pinned_and_still_runs() -> eyre::Result<()> {
        let formatted = format(include_str!("loadable.sst"), Default::default());
        assert_eq!(formatted, FORMATTED);
        assert_eq!(format(&formatted, Default::default()), formatted);
        assert_eq!(check(&formatted), Vec::<String>::new());
        let mut runtime = Runtime::from_sst(&formatted)?;
        runtime.controller.execute(&[])?;
        assert_eq!(runtime.controller.machine().execution.stack(), &[42]);

        let plain = format(
            &formatted,
            vihaco_sst::FormatOptions::without_generated_comments(),
        );
        assert!(!plain.contains("//!"));
        assert!(plain.contains("                name: 1,   // strings[1]: helper\n"));
        let mut runtime = Runtime::from_sst(&plain)?;
        runtime.controller.execute(&[])?;
        assert_eq!(runtime.controller.machine().execution.stack(), &[42]);
        Ok(())
    }

    #[test]
    fn the_example_source_checks_clean_without_executing() {
        assert_eq!(check(include_str!("loadable.sst")), Vec::<String>::new());
    }

    #[test]
    fn the_checker_reports_every_problem_in_a_broken_source() {
        let source = include_str!("loadable.sst")
            .replace("@helper:", "@helpr:")
            .replace("br @done", "br @finish")
            .replace("main_function: 1,", "main_function: 2,");
        assert_eq!(
            check(&source),
            [
                "11:23: error: function `@helper` (metadata.functions[0]) has no `@helper:` \
                 symbol definition",
                "24:24: error: metadata.main_function: function index 2 is out of range \
                 (metadata.functions has 2 entries)",
                "30:33: error: function `@helper` has no `@helper:` symbol definition",
                "33:28: error: undefined symbol `@finish`",
            ]
        );
    }

    #[test]
    fn duplicate_declarations_are_rejected_during_collection() {
        let source = include_str!("loadable.sst").replace(
            "experiment.description \"Loadable SST example: 21 * 2\";",
            "experiment.description \"first\"; experiment.description \"second\";",
        );
        let error = Runtime::from_sst(&source)
            .err()
            .expect("duplicate descriptions must be rejected");
        assert!(
            error
                .to_string()
                .contains("duplicate description declaration")
        );
    }
}
