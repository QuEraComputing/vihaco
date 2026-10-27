// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Run with:
//! `cargo run -p vihaco-stdlib --features program-image-execution --example loadable`
//!
//! Pass an SST file path as the final argument to load another source file.
//! Without a path, this runs the adjacent `loadable.sst`, which computes 21 * 2.

use vihaco_abi::{Type, Value};
use vihaco_bytecode::{NoContext, SstFile};
use vihaco_cpu::CPU;
use vihaco_module::{
    loader::MetadataLoweringContext,
    module::{LocalModule, ProgramMetadata},
};
use vihaco_runtime::{Apply, BuildMessage, Execute, HasInstructionSet, Step, composite, field};
use vihaco_stdlib::{ExecutionState, Image, Loadable};
use vihaco_syntax::{HeaderDeclarations, ParsedModule, lower_functions_with_metadata};

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
        let file = SstFile::<NoContext>::from_text(source)?;
        let section = file
            .root()
            .child("controller")
            .ok_or_else(|| eyre::eyre!("missing controller section"))?;

        // 2. Parse header declarations and the composite's syntax instructions.
        let ParsedModule { header, functions } = ParsedModule::<
            SourceInstruction,
            Type,
            HeaderDeclarations<HeaderDeclaration>,
        >::parse_section(section)?;

        // 3. Collect declarations and reject duplicates, then check the required
        // metadata and description for this program.
        let mut header_state = HeaderDeclarationsState::default();
        for declaration in header {
            declaration.collect_into(&mut header_state)?;
        }
        let (metadata, info) = header_state.finish_program()?;

        // 4. Build symbol capabilities, validate declarations/tables, and lower
        // function bodies. The driver prepares labels before lowering each body.
        let mut context = MetadataLoweringContext::new(&metadata)?;
        let code = lower_functions_with_metadata(functions, &metadata, &mut context)?;
        let module = metadata.map_extra(|_| info).into_module(code)?;

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
