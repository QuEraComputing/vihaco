// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Run with:
//! `cargo run -p vihaco-stdlib --features program-image-execution --example heterogeneous`
//!
//! One SST file, two machines: the `controller` section targets a CPU-style
//! machine and the `fpga` section a pulse-generator machine with its own
//! dialect. The example checks and formats the file with one machine per
//! section, loads both sections, disassembles the loaded programs back into
//! one SST file, and reloads that file to show nothing changed.

use vihaco_abi::{Type, Value};
use vihaco_bytecode::{NoContext, SSTFile};
use vihaco_module::{
    loader::MetadataLoweringContext,
    module::{LocalModule, ProgramDirective},
};
use vihaco_parser::Parse;
use vihaco_runtime::{Execute, HasInstructionSet, Lower, NoMessage, component, composite, dialect};
use vihaco_sst::{Disassembler, FileChecker, FileFormatter, FormatOptions, SourceFile};
use vihaco_syntax::{HeaderDeclarations, ParsedModule};

/// The controller: program control plus CPU arithmetic.
#[composite]
struct Controller {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    cpu: vihaco_cpu::CPU,
}

/// Pulse instructions for the FPGA.
#[dialect]
pub mod pulse {
    /// Play a pulse on a channel for a duration, in clock cycles.
    #[instruction]
    #[pattern = "'play $0 `,` $1"]
    pub struct Play(pub u32, pub u64);

    /// Wait for a number of clock cycles.
    #[instruction]
    pub struct Wait(pub u64);
}

/// A pulse generator that records what it would play.
#[component]
#[instructions { pulse }]
#[derive(Default)]
pub struct Pulser {
    played: Vec<(u32, u64)>,
}

impl Execute<pulse::Play> for Pulser {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &pulse::Play, _: NoMessage) -> eyre::Result<()> {
        self.played.push((instruction.0, instruction.1));
        Ok(())
    }
}

impl Execute<pulse::Wait> for Pulser {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    fn execute(&mut self, _: &pulse::Wait, _: NoMessage) -> eyre::Result<()> {
        Ok(())
    }
}

/// The FPGA: program control plus the pulse generator.
#[composite]
struct Fpga {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    pulser: Pulser,
}

type Program<M> = LocalModule<<M as HasInstructionSet>::Runtime, Value, Type>;

/// Load section `name` of `file` as a program for machine `M`.
fn load<'src, M>(file: &'src SSTFile<NoContext>, name: &str) -> eyre::Result<Program<M>>
where
    M: HasInstructionSet,
    M::Syntax: Parse<'src> + 'src + Lower<MetadataLoweringContext, Runtime = M::Runtime>,
{
    let section = file
        .section(name)
        .ok_or_else(|| eyre::eyre!("missing {name} section"))?;
    let parsed =
        ParsedModule::<M::Syntax, HeaderDeclarations<ProgramDirective>>::parse_section(section)?;
    let ParsedModule {
        header,
        code,
        symbols,
    } = parsed;
    let metadata = header
        .into_iter()
        .collect::<vihaco_module::module::ProgramHeader>()
        .finish(vihaco_module::module::NoInfo)?;
    let mut context = MetadataLoweringContext::new(&metadata, &symbols)?;
    ParsedModule {
        header: (),
        code,
        symbols,
    }
    .lower_with_metadata(metadata, &mut context)
}

/// Both loaded sections of an SST file.
struct Programs {
    controller: Program<Controller>,
    fpga: Program<Fpga>,
}

impl Programs {
    fn load(source: &str) -> eyre::Result<Self> {
        let file = SSTFile::<NoContext>::from_text(source)?;
        Ok(Self {
            controller: load::<Controller>(&file, "controller")?,
            fpga: load::<Fpga>(&file, "fpga")?,
        })
    }

    /// Disassemble both programs back into one SST file.
    fn disassemble(&self, options: FormatOptions) -> eyre::Result<String> {
        Disassembler::new()
            .section::<Controller>("controller", &self.controller)
            .section::<Fpga>("fpga", &self.fpga)
            .finish(options)
    }
}

fn checker() -> FileChecker {
    FileChecker::new()
        .section::<Controller>("controller")
        .section::<Fpga>("fpga")
}

fn formatter() -> FileFormatter {
    FileFormatter::new()
        .section::<Controller>("controller")
        .section::<Fpga>("fpga")
}

fn main() -> eyre::Result<()> {
    let text = include_str!("heterogeneous.sst");
    let source = SourceFile::new("heterogeneous.sst", text);

    // 1. Check and format, choosing each section's machine by its name.
    checker().check(&source).into_result(&source)?;
    let formatted = formatter()
        .format(&source)
        .map_err(|report| eyre::eyre!("{}", report.render(&source)))?;

    // 2. Load each section for its own machine.
    let programs = Programs::load(&formatted)?;
    println!(
        "Loaded {} controller and {} fpga instructions",
        programs.controller.code.len(),
        programs.fpga.code.len()
    );

    // 3. Disassemble both loaded programs, and reload the result.
    let disassembly = programs.disassemble(FormatOptions::default())?;
    print!("{disassembly}");
    let reloaded = Programs::load(&disassembly)?;
    eyre::ensure!(
        reloaded.controller == programs.controller,
        "controller changed"
    );
    eyre::ensure!(reloaded.fpga == programs.fpga, "fpga changed");
    println!(";; reloaded: both programs are unchanged");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_runs() -> eyre::Result<()> {
        main()
    }

    #[test]
    fn each_section_is_checked_with_its_own_machine() {
        // Replace the controller's CPU instruction before inserting it in the FPGA section.
        let source = include_str!("heterogeneous.sst")
            .replace("cpu::cpu::mul_u64", "pulser::pulse::wait 1")
            .replace("pulser::pulse::wait 10", "cpu::cpu::mul_u64");
        let source = SourceFile::new("heterogeneous.sst", source);
        let messages: Vec<String> = checker()
            .check(&source)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            messages,
            [
                "16:5: error: expected an instruction, found `pulser::pulse::wait`",
                "27:5: error: expected an instruction, found `cpu::cpu::mul_u64`",
            ]
        );
    }

    #[test]
    fn unregistered_and_missing_sections_are_errors() {
        let source = include_str!("heterogeneous.sst").replace(".section fpga", ".section dsp");
        let source = SourceFile::new("heterogeneous.sst", source);
        let messages: Vec<String> = checker()
            .check(&source)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            messages,
            [
                "1:1: error: missing section `fpga`",
                "19:10: error: no machine is registered for section `dsp`",
            ]
        );
        // A default machine covers every unlisted section.
        let mut checker = FileChecker::new()
            .section::<Controller>("controller")
            .default_section::<Fpga>();
        assert!(checker.check(&source).is_ok());
    }

    #[test]
    fn formatting_is_idempotent_and_matches_the_disassembly() -> eyre::Result<()> {
        let source = SourceFile::new("heterogeneous.sst", include_str!("heterogeneous.sst"));
        let formatted = formatter().format(&source).unwrap();
        let again = SourceFile::new("heterogeneous.sst", formatted.as_str());
        assert_eq!(formatter().format(&again).unwrap(), formatted);
        // Without the user comments, the formatted file is the disassembly.
        let uncommented: String = formatted
            .lines()
            .filter(|line| !line.starts_with("; "))
            .map(|line| format!("{line}\n"))
            .collect();
        let disassembly = Programs::load(&formatted)?.disassemble(FormatOptions::default())?;
        assert_eq!(disassembly, uncommented);
        Ok(())
    }
}
