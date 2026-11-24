// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Run with:
//! `cargo run -p vihaco-stdlib --example loadable`
//!
//! Pass an SST file path as the final argument to load another source file.
//! Without a path, this runs the adjacent `loadable.sst`, which computes 21 * 2.
//!
//! Instead of running, the file can be checked or formatted for this machine:
//!
//! - `--check`: report every problem, with line and column, and run nothing.
//! - `--fmt`: print the canonical layout with generated `;;` notes.
//! - `--fmt-plain`: the same, without generated notes.
//! - `--disasm`: load the file, then disassemble the loaded program back to
//!   SST and print it.

use vihaco_abi::{Type, Value};
use vihaco_bytecode::{NoContext, SSTFile};
use vihaco_cpu::CPU;
use vihaco_module::{
    loader::MetadataLoweringContext,
    module::{LocalModule, ProgramDirective, ProgramHeader, StandardDirective},
    raising::ExtraDirectives,
};
use vihaco_runtime::{
    Apply, BuildMessage, Execute, HasInstructionSet, ReusableMessageRecipe, Step, composite, field,
};
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
impl<I, M> BuildMessage<field!(Machine::execution), I, Program, M> for Machine
where
    ExecutionState: Execute<I, Message = M>,
    M: ReusableMessageRecipe<Self, field!(Machine::execution), I, Program>,
{
    fn build_message(&mut self, instruction: &I, context: &Program) -> eyre::Result<M> {
        <M as ReusableMessageRecipe<Self, field!(Machine::execution), I, Program>>::prepare(
            self,
            instruction,
            context,
        )
    }
}

impl<I, M> BuildMessage<field!(Machine::cpu), I, Program, M> for Machine
where
    CPU: Execute<I, Message = M>,
    M: ReusableMessageRecipe<Self, field!(Machine::cpu), I, Program>,
{
    fn build_message(&mut self, instruction: &I, context: &Program) -> eyre::Result<M> {
        <M as ReusableMessageRecipe<Self, field!(Machine::cpu), I, Program>>::prepare(
            self,
            instruction,
            context,
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

#[derive(Debug, Clone, PartialEq)]
struct ExperimentInfo {
    description: String,
}

// The disassembler writes the loaded description back as its directive.
impl ExtraDirectives<HeaderDeclaration> for ExperimentInfo {
    fn extra_directives(&self) -> eyre::Result<Vec<HeaderDeclaration>> {
        Ok(vec![HeaderDeclaration::Description(
            self.description.clone(),
        )])
    }
}

// A header is a list of directives, one per line. The standard directives
// (`.file`, `.func`, `.entry`, `.const`, `.source_symbol`) describe the
// program; `experiment.description` is this application's own directive.
#[derive(Debug, vihaco_parser_derive::Parse)]
#[syntax_class(metadata)]
enum HeaderDeclaration {
    #[pattern = "$0"]
    Standard(ProgramDirective<Value, Type>),
    #[pattern = "experiment.description $0"]
    Description(String),
}

// Lets the disassembler write standard directives into this header.
impl From<ProgramDirective<Value, Type>> for HeaderDeclaration {
    fn from(directive: ProgramDirective<Value, Type>) -> Self {
        Self::Standard(directive)
    }
}

// Lets the checker and formatter find the standard directives.
impl StandardDirective for HeaderDeclaration {
    type Value = Value;
    type Type = Type;

    fn as_standard(&self) -> Option<&ProgramDirective<Value, Type>> {
        match self {
            Self::Standard(directive) => Some(directive),
            Self::Description(_) => None,
        }
    }
}

#[derive(Default)]
struct HeaderDeclarationsState {
    program: ProgramHeader<Value, Type>,
    description: Option<String>,
}

fn record_description<T: AsRef<str>>(seen: &mut Option<T>, description: T) -> eyre::Result<()> {
    eyre::ensure!(seen.is_none(), "duplicate description declaration");
    let empty = description.as_ref().is_empty();
    // An empty declaration still counts when checking later declarations.
    *seen = Some(description);
    eyre::ensure!(!empty, "experiment.description must not be empty");
    Ok(())
}

fn require_description<T>(description: Option<T>) -> eyre::Result<T> {
    description.ok_or_else(|| eyre::eyre!("missing experiment.description"))
}

impl HeaderDeclaration {
    fn collect_into(self, declarations: &mut HeaderDeclarationsState) -> eyre::Result<()> {
        match self {
            // Standard directives are assembled in header order.
            Self::Standard(directive) => declarations.program.push(directive),
            Self::Description(description) => {
                record_description(&mut declarations.description, description)?;
            }
        }
        Ok(())
    }
}

impl HeaderDeclarationsState {
    /// Assemble the program tables, with the description as `extra` metadata.
    fn finish_program(
        self,
    ) -> eyre::Result<vihaco_module::module::ProgramMetadata<Value, Type, ExperimentInfo>> {
        let description = require_description(self.description)?;
        self.program.finish(ExperimentInfo { description })
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

        // 3. Collect declarations and reject duplicates, then assemble the
        // program tables and require the description.
        let mut header_state = HeaderDeclarationsState::default();
        for declaration in header {
            declaration.collect_into(&mut header_state)?;
        }
        let mut metadata = header_state.finish_program()?;

        // 4. Build symbol capabilities from the metadata and the section's symbol
        // table, validate declarations/tables, and lower the code stream.
        // Lowering interns string literals after the header's strings.
        let mut context = MetadataLoweringContext::new(&metadata, &symbols)?;
        let code = lower_code_with_metadata(code, &symbols, &metadata, &mut context)?;
        metadata.strings = context.into_strings();
        let module = metadata.into_module(code, symbols)?;

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

/// What `main` does with the source file.
enum Mode {
    Run,
    Check,
    Format(vihaco_sst::FormatOptions),
    Disassemble,
}

/// Disassemble the loaded controller program back to SST.
fn disassemble(program: &Program) -> eyre::Result<String> {
    vihaco_sst::Disassembler::new()
        .section_with::<Machine, HeaderDeclaration>("controller", program)
        .finish(vihaco_sst::FormatOptions::default())
}

fn validate_header(
    header: &vihaco_sst::Declarations<HeaderDeclaration>,
    section: &vihaco_sst::check::Section<'_>,
    out: &mut vihaco_sst::Diagnostics<'_>,
) {
    let mut description = None;
    for declaration in &header.0 {
        if let HeaderDeclaration::Description(value) = &declaration.value
            && let Err(error) = record_description(&mut description, value.as_str())
        {
            out.error(
                declaration.span.clone(),
                error.to_string(),
                "in this declaration",
            );
        }
    }
    if let Err(error) = require_description(description) {
        out.error(
            section.block_anchor(section.header()),
            error.to_string(),
            "in this header",
        );
    }
}

fn checker() -> vihaco_sst::ProgramChecker<Machine, vihaco_sst::Declarations<HeaderDeclaration>> {
    vihaco_sst::ProgramChecker::<Machine>::new()
        .with_header::<vihaco_sst::Declarations<HeaderDeclaration>>()
        .with_header_validation(validate_header)
        .require_sections(["controller"])
}

fn main() -> eyre::Result<()> {
    let mut mode = Mode::Run;
    let mut path = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--check" => mode = Mode::Check,
            "--fmt" => mode = Mode::Format(vihaco_sst::FormatOptions::default()),
            "--fmt-plain" => {
                mode = Mode::Format(vihaco_sst::FormatOptions::without_generated_comments())
            }
            "--disasm" => mode = Mode::Disassemble,
            flag if flag.starts_with("--") => return Err(eyre::eyre!("unknown flag `{flag}`")),
            _ => path = Some(arg),
        }
    }
    let (name, source) = match &path {
        Some(path) => (path.clone(), std::fs::read_to_string(path)?),
        None => (
            "loadable.sst".to_owned(),
            include_str!("loadable.sst").to_owned(),
        ),
    };

    match mode {
        Mode::Run => {}
        Mode::Check => {
            let file = vihaco_sst::SourceFile::new(name, source);
            let report = checker().check(&file);
            if report.is_ok() {
                eprintln!("no problems found");
                return Ok(());
            }
            eprint!("{}", report.render(&file));
            std::process::exit(1);
        }
        Mode::Format(options) => {
            let file = vihaco_sst::SourceFile::new(name, source);
            match vihaco_sst::Formatter::with_checker(checker())
                .options(options)
                .format(&file)
            {
                Ok(formatted) => {
                    print!("{formatted}");
                    return Ok(());
                }
                Err(report) => {
                    eprint!("{}", report.render(&file));
                    std::process::exit(1);
                }
            }
        }
        Mode::Disassemble => {
            let runtime = Runtime::from_sst(&source)?;
            print!("{}", disassemble(&runtime.controller.image().module)?);
            return Ok(());
        }
    }
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

    fn check(source: &str) -> Vec<String> {
        let source = vihaco_sst::SourceFile::new("loadable.sst", source);
        checker()
            .check(&source)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// `loadable.sst` in the formatter's canonical layout.
    const FORMATTED: &str = r#"sst v2

.section controller
.header
    experiment.description "Loadable SST example: 21 * 2"
    .file "loadable.sst"
    ; Function table order is independent of executable function order.
    ;; functions[0], @helper = 0x0006..0x0008
    .func @helper() -> u64, locals 0
    ;; functions[1], @main = 0x0000..0x0006
    .func @main() -> u64, locals 0
    .entry @main  ; a function, not an instruction address
.text
;; fn main() -> u64, locals 0, functions[1]
@main:
    ;; @helper = 0x0006 (functions[0])
    execution::program::call 0, @helper
    execution::program::const_u64 2
    cpu::cpu::mul_u64
    ;; @done = 0x0005
    execution::program::br @done
    execution::program::const_u64 999
;; 0x0005
@done:
    execution::program::ret 1
;; fn helper() -> u64, locals 0, functions[0]
@helper:
    execution::program::const_u64 21
    execution::program::ret 1
"#;

    fn format(text: &str, options: vihaco_sst::FormatOptions) -> String {
        let source = vihaco_sst::SourceFile::new("loadable.sst", text);
        vihaco_sst::Formatter::with_checker(checker())
            .options(options)
            .format(&source)
            .unwrap_or_else(|report| panic!("{}", report.render(&source)))
    }

    #[test]
    fn description_rules_match_when_loading_checking_and_formatting() {
        let original = include_str!("loadable.sst");
        let original_description = "    experiment.description \"Loadable SST example: 21 * 2\"";
        for (replacement, location, message) in [
            ("", "4:1", "missing experiment.description"),
            (
                "    experiment.description \"\"",
                "5:5",
                "experiment.description must not be empty",
            ),
            (
                "    experiment.description \"first\"\n    experiment.description \"second\"",
                "6:5",
                "duplicate description declaration",
            ),
        ] {
            let text = original.replacen(original_description, replacement, 1);
            let load_error = Runtime::from_sst(&text).err().expect("invalid description");
            assert_eq!(load_error.to_string(), message);
            let source = vihaco_sst::SourceFile::new("loadable.sst", text);
            let report = checker().check(&source);
            assert_eq!(report.diagnostics.len(), 1);
            assert_eq!(
                report.diagnostics[0].to_string(),
                format!("{location}: error: {message}")
            );
            for options in [
                vihaco_sst::FormatOptions::default(),
                vihaco_sst::FormatOptions::without_generated_comments(),
            ] {
                assert_eq!(
                    vihaco_sst::Formatter::with_checker(checker())
                        .options(options)
                        .format(&source)
                        .unwrap_err(),
                    report
                );
            }
        }
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
        assert!(!plain.contains(";;"));
        assert!(plain.contains("\n    .entry @main  ; a function, not an instruction address\n"));
        let mut runtime = Runtime::from_sst(&plain)?;
        runtime.controller.execute(&[])?;
        assert_eq!(runtime.controller.machine().execution.stack(), &[42]);
        Ok(())
    }

    #[test]
    fn the_unformatted_example_formats_to_the_pinned_layout() {
        let formatted = format(include_str!("loadable-unformatted.sst"), Default::default());
        // Only the leading "Try:" comments differ from `loadable.sst`.
        let (preamble, rest) = formatted.split_once(".section").unwrap();
        assert!(
            preamble.contains("; A hand-written, messy copy"),
            "{formatted}"
        );
        assert_eq!(rest, FORMATTED.split_once(".section").unwrap().1);
    }

    #[test]
    fn the_broken_example_reports_every_mistake() {
        assert_eq!(
            check(include_str!("loadable-broken.sst")),
            [
                "8:11: error: function `@helper` (functions[0]) has no `@helper:` symbol \
                 definition",
                "10:12: error: `.entry @mian` names no `.func` directive",
                "13:33: error: function `@helper` has no `@helper:` symbol definition",
                "16:28: error: undefined symbol `@finish`",
            ]
        );
    }

    /// `FORMATTED` without its user comments: what disassembly writes.
    fn uncommented() -> String {
        FORMATTED
            .replace(
                "    ; Function table order is independent of executable function order.\n",
                "",
            )
            .replace("  ; a function, not an instruction address", "")
    }

    #[test]
    fn disassembly_reloads_to_the_same_program() -> eyre::Result<()> {
        for source in [
            include_str!("loadable.sst"),
            include_str!("loadable-unformatted.sst"),
        ] {
            let runtime = Runtime::from_sst(source)?;
            let module = &runtime.controller.image().module;
            let text = disassemble(module)?;
            assert_eq!(text, uncommented());
            let mut reloaded = Runtime::from_sst(&text)?;
            assert_eq!(&reloaded.controller.image().module, module);
            reloaded.controller.execute(&[])?;
            assert_eq!(reloaded.controller.machine().execution.stack(), &[42]);
        }
        Ok(())
    }

    #[test]
    fn branch_targets_without_symbols_get_synthesized_labels() -> eyre::Result<()> {
        use vihaco_module::module::Symbol;

        let runtime = Runtime::from_sst(include_str!("loadable.sst"))?;
        let mut module = runtime.controller.image().module.clone();
        // Drop `@done` (a branch target) and `@helper` (a function), and take
        // the name a synthesized label would get.
        module
            .symbols
            .retain(|symbol| symbol.name != "done" && symbol.name != "helper");
        module.symbols.push(Symbol {
            name: "L0005".into(),
            address: 8,
        });
        let text = disassemble(&module)?;
        assert!(
            text.contains("\n    execution::program::br @L0005_1\n"),
            "{text}"
        );
        assert!(
            text.contains("\n@L0005_1:\n    execution::program::ret 1\n"),
            "{text}"
        );
        assert!(text.contains("\n@helper:\n"), "{text}");
        assert!(text.ends_with("\n@L0005:\n"), "{text}");

        let reloaded = Runtime::from_sst(&text)?;
        let reloaded = &reloaded.controller.image().module;
        let names: Vec<(&str, u32)> = reloaded
            .symbols
            .iter()
            .map(|symbol| (symbol.name.as_str(), symbol.address))
            .collect();
        assert_eq!(
            names,
            [("main", 0), ("L0005_1", 5), ("helper", 6), ("L0005", 8)]
        );
        assert_eq!(reloaded.code, module.code);
        assert_eq!(reloaded.functions, module.functions);
        assert_eq!(reloaded.constants, module.constants);
        assert_eq!(reloaded.strings, module.strings);
        assert_eq!(reloaded.main_function, module.main_function);
        Ok(())
    }

    #[test]
    fn programs_that_sst_cannot_express_are_rejected() -> eyre::Result<()> {
        let runtime = Runtime::from_sst(include_str!("loadable.sst"))?;
        let mut module = runtime.controller.image().module.clone();
        // `@helper` ends at the code length, so it cannot end earlier.
        module.functions[0].end_address = 7;
        let error = disassemble(&module).unwrap_err();
        assert!(
            format!("{error:#}").contains("functions[0] ends at 0x0007"),
            "{error:#}"
        );

        let mut module = runtime.controller.image().module.clone();
        // A function whose symbol is somewhere else.
        module.functions[0].start_address = 7;
        let error = disassemble(&module).unwrap_err();
        assert!(
            format!("{error:#}")
                .contains("function `@helper` starts at 0x0007, but symbol `@helper` is at 0x0006"),
            "{error:#}"
        );
        // Without that symbol, the call at 0x0000 targets no function.
        module.symbols.retain(|symbol| symbol.name != "helper");
        let error = disassemble(&module).unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            "cannot disassemble section `controller`: cannot raise the instruction at 0x0000: \
             no function starts at address 0x0006"
        );
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
            .replace(".entry @main", ".entry @mian");
        assert_eq!(
            check(&source),
            [
                "8:11: error: function `@helper` (functions[0]) has no `@helper:` symbol \
                 definition",
                "10:12: error: `.entry @mian` names no `.func` directive",
                "13:33: error: function `@helper` has no `@helper:` symbol definition",
                "16:28: error: undefined symbol `@finish`",
            ]
        );
    }

    #[test]
    fn duplicate_declarations_are_rejected_during_collection() {
        let source = include_str!("loadable.sst").replace(
            "experiment.description \"Loadable SST example: 21 * 2\"",
            "experiment.description \"first\"\nexperiment.description \"second\"",
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
