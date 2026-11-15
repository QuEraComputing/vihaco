// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Doc tests for the vihaco documentation site.
//!
//! This crate ships no real code. It exists so CI fails if the documentation
//! drifts from the actual API:
//!
//! - The example files shown on the landing and quick-start pages
//!   (`docs/examples/*.rs`) are `include!`d below, so they are compiled exactly
//!   as the site shows them (and the runnable ones are executed by `#[test]`).
//! - Every fenced ` ```rust ` block in the guide markdown is a rustdoc doctest;
//!   run them with `cargo test --doc -p vihaco-doctests --features program-image-execution`.
//!   Blocks that are
//!   genuinely illustrative (trait reproductions, runtime pseudocode that calls
//!   helpers a real consumer would write) are marked `ignore`; blocks that
//!   compile but should not run are marked `no_run`.
//!
//! The example files define `pub` items that nothing else uses, hence the
//! crate-wide `dead_code` allowance.
#![allow(dead_code, unused_imports, unused_variables)]

// ── Landing / quick-start example files (compiled as shown on the site) ──

#[cfg(feature = "program-image-execution")]
mod ex_counter {
    include!("../../../docs/examples/counter.rs");
}

#[cfg(feature = "program-image-execution")]
mod ex_quickstart {
    include!("../../../docs/examples/quickstart.rs");

    #[test]
    fn runs() {
        main().expect("quickstart example should run");
    }

    /// Printing is the inverse of parsing for the quick-start section.
    #[test]
    fn first_machine_sst_prints_back_to_its_source() {
        use vihaco::chumsky::Parser as _;
        use vihaco::{Parse, Print};

        let source = include_str!("../../../docs/examples/first-machine.sst");
        let file = SSTFile::<NoContext>::from_text(source).unwrap();
        let parsed =
            ParsedModule::<SourceInstruction, HeaderDeclarations<ProgramDirective>>::parse_section(
                file.section("main").unwrap(),
            )
            .unwrap();

        let header = source.split(".header\n").nth(1).unwrap();
        let header = header.split(".text").next().unwrap();
        let printed: Vec<_> = parsed
            .header
            .0
            .iter()
            .map(|directive| {
                let printed = directive.to_source().unwrap();
                let reparsed = ProgramDirective::parser()
                    .parse(printed.as_str())
                    .into_result()
                    .unwrap();
                assert_eq!(&reparsed, directive, "{printed}");
                printed
            })
            .collect();
        assert_eq!(printed, header.lines().map(str::trim).collect::<Vec<_>>());

        let text = source.split(".text").nth(1).unwrap();
        let lines: Vec<_> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('@'))
            .collect();
        let printed: Vec<_> = parsed
            .code
            .iter()
            .map(|instruction| {
                let printed = instruction.to_source().unwrap();
                let reparsed = SourceInstruction::parser()
                    .parse(printed.as_str())
                    .into_result()
                    .unwrap();
                assert_eq!(&reparsed, instruction, "{printed}");
                printed
            })
            .collect();
        assert_eq!(printed, lines);
    }

    /// `first-machine.sst` in the formatter's canonical layout.
    const FORMATTED: &str = "\
sst v2

.section main
.header
    .file \"first-machine.sst\"
    ;; functions[0], @main = 0x0000..0x0004
    .func @main() -> u64, locals 0
    .entry @main
.text
;; fn main() -> u64, locals 0, functions[0]
@main:
    execution::program::const_u64 20
    execution::program::const_u64 22
    calculator::arithmetic::add
    execution::program::ret 1
";

    /// Load the `main` section of `source`.
    fn load(source: &str) -> eyre::Result<Program> {
        let file = SSTFile::<NoContext>::from_text(source)?;
        let section = file
            .section("main")
            .ok_or_else(|| eyre::eyre!("missing main section"))?;
        let ParsedModule {
            header,
            code,
            symbols,
        } = ParsedModule::<SourceInstruction, HeaderDeclarations<ProgramDirective>>::parse_section(
            section,
        )?;
        let mut metadata = header
            .into_iter()
            .collect::<ProgramHeader>()
            .finish(NoInfo)?;
        let mut context = MetadataLoweringContext::new(&metadata, &symbols)?;
        let code = lower_code_with_metadata(code, &symbols, &metadata, &mut context)?;
        metadata.strings = context.into_strings();
        metadata.into_module(code, symbols)
    }

    /// Load and run the `main` section of `source`, returning the stack.
    fn run(source: &str) -> eyre::Result<Vec<u64>> {
        let module = load(source)?;
        let machine = Machine {
            execution: ExecutionState::new(),
            calculator: Calculator::default(),
        };
        let mut loaded = Loadable::new(Image::new(module), machine);
        loaded.execute(&[])?;
        Ok(loaded.machine().execution.stack().to_vec())
    }

    #[test]
    fn first_machine_sst_formats_to_a_pinned_layout_that_still_runs() {
        use vihaco_sst::{FormatOptions, SourceFile, format_program};

        let format = |text: &str| {
            let source = SourceFile::new("first-machine.sst", text);
            format_program::<Machine>(&source, FormatOptions::default())
                .unwrap_or_else(|report| panic!("{}", report.render(&source)))
        };
        let formatted = format(include_str!("../../../docs/examples/first-machine.sst"));
        assert_eq!(formatted, FORMATTED);
        assert_eq!(format(&formatted), formatted);
        assert_eq!(run(&formatted).unwrap(), [42]);
    }

    /// Disassembling the loaded program gives the formatted file back, and
    /// it loads to the same program.
    #[test]
    fn first_machine_sst_disassembles_to_its_formatted_layout() -> eyre::Result<()> {
        use vihaco_sst::{Disassembler, FormatOptions};

        let module = load(include_str!("../../../docs/examples/first-machine.sst"))?;
        let text = Disassembler::new()
            .section::<Machine>("main", &module)
            .finish(FormatOptions::default())?;
        assert_eq!(text, FORMATTED);
        assert_eq!(load(&text)?, module);
        Ok(())
    }
}

#[cfg(feature = "program-image-execution")]
mod ex_patterns {
    include!("../../../docs/examples/patterns.rs");

    #[test]
    fn runs() {
        main().expect("pattern examples should run");
    }
}

#[cfg(feature = "program-image-execution")]
mod ex_quickstart_parse {
    include!("../../../docs/examples/quickstart_parse.rs");

    #[test]
    fn runs() {
        main();
    }
}

// ── Guide tutorials (every ```rust block is a rustdoc doctest) ──

#[cfg(feature = "program-image-execution")]
mod authors {
    #[doc = include_str!("../../../docs/src/pages/guide/mental-model.md")]
    mod mental_model {}

    #[doc = include_str!("../../../docs/src/pages/guide/first-machine.md")]
    mod first_machine {}

    #[doc = include_str!("../../../docs/src/pages/guide/essentials.md")]
    mod essentials {}

    #[doc = include_str!("../../../docs/src/pages/guide/sst-reference.md")]
    mod sst_reference {}

    #[doc = include_str!("../../../docs/src/pages/guide/execution.md")]
    mod execution {}

    #[doc = include_str!("../../../docs/src/pages/guide/stack.md")]
    mod stack {}

    #[doc = include_str!("../../../docs/src/pages/guide/program-execution.md")]
    mod program_execution {}

    #[doc = include_str!("../../../docs/src/pages/guide/loadables.md")]
    mod loadables {}

    #[doc = include_str!("../../../docs/src/pages/guide/patterns.md")]
    mod patterns {}

    #[doc = include_str!("../../../docs/src/pages/guide/testing.md")]
    mod testing {}

    #[doc = include_str!("../../../docs/src/pages/guide/instructions.md")]
    mod guide_instructions {}

    #[doc = include_str!("../../../docs/src/pages/guide/instructions-advanced.md")]
    mod guide_instructions_advanced {}

    #[doc = include_str!("../../../docs/src/pages/guide/parser.md")]
    mod guide_parser {}

    #[doc = include_str!("../../../docs/src/pages/guide/parser-patterns.md")]
    mod guide_parser_patterns {}

    #[doc = include_str!("../../../docs/src/pages/guide/parser-advanced.md")]
    mod guide_parser_advanced {}

    #[doc = include_str!("../../../docs/src/pages/guide/messages.md")]
    mod guide_messages {}

    #[doc = include_str!("../../../docs/src/pages/guide/components.md")]
    mod guide_components {}

    #[doc = include_str!("../../../docs/src/pages/guide/effects.md")]
    mod guide_effects {}

    #[doc = include_str!("../../../docs/src/pages/guide/composites.md")]
    mod guide_composites {}
}
