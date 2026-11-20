// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Fixture-style checker tests: each malformed file asserts the exact set of
//! diagnostics, as `line:column: message`.

mod common;

use common::{Machine, header, source, summary};
use vihaco_module::loader::MetadataLoweringContext;
use vihaco_module::module::{ProgramDirective, ProgramMetadata, StandardDirective};
use vihaco_sst::{Declarations, ProgramChecker, check_framing, check_program};

fn check(text: &str) -> Vec<String> {
    summary(&check_program::<Machine>(&source(text)))
}

/// A one-section file whose header declares `functions` (the first is the
/// entry `main`). `.text` starts on line `6 + functions.len()`.
fn program(functions: &[&str], text: &str) -> String {
    format!(
        "sst v2\n.section main\n.header\n{}.text\n{text}",
        header(functions)
    )
}

#[test]
fn a_well_formed_program_checks_clean() {
    let text = program(
        &["main", "helper"],
        "@main:\n    t.call 0, @helper\n    t.print \"hi\"\n    t.br @done\n@done: t.halt\n\
         @helper:\n    t.push 21 ; the answer, halved\n    t.halt\n",
    );
    assert_eq!(check(&text), Vec::<String>::new());
}

#[test]
fn framing_reports_every_error() {
    let text = "sst v2\nstray content\nmore stray\n.section main\n.header\n.header\n.text\n\
                .section main\n.global\n.section\n.section a b\n";
    let report = check_framing(&source(text));
    assert_eq!(
        summary(&report),
        [
            "2:1: unexpected content; expected `.global` or `.section <name>`",
            "6:1: section `main` declares duplicate `.header`",
            "8:10: duplicate section `main`",
            "9:1: `.global` must come before any `.section`",
            "10:1: `.section` expects a section name",
            "11:10: `.section` expects a single name, found `a b`",
        ]
    );
    let source = source(text);
    let secondary: Vec<_> = report
        .iter()
        .flat_map(|diagnostic| &diagnostic.secondary)
        .map(|label| {
            (
                source.location(label.span.start).to_string(),
                label.message.as_str(),
            )
        })
        .collect();
    assert_eq!(
        secondary,
        [
            ("5:1".to_owned(), "first `.header` here"),
            ("4:10".to_owned(), "first defined here"),
        ]
    );
}

#[test]
fn version_line_problems() {
    for (text, expected) in [
        ("", "1:1: expected `sst v2`"),
        ("\n.section main\n", "2:1: expected `sst v2`"),
        (
            "sst v3\n.section main\n",
            "1:1: unsupported sst version 3 (expected 2)",
        ),
    ] {
        assert_eq!(
            summary(&check_framing(&source(text))),
            [expected],
            "{text:?}"
        );
    }
}

#[test]
fn header_syntax_errors_are_all_reported() {
    let text = "sst v2\n.section main\n.header\n.func @main(), locals 0\n.fnc @x(), locals 0\n\
                .func @y() -> u64\nbogus ; old terminator\n.func @z(\n.entry @main .entry @main\n\
                .text\n@main:\n    t.halt\n";
    assert_eq!(
        check(text),
        [
            "5:1: unknown directive `.fnc`",
            "6:18: expected the rest of the `.func` declaration, found end of line",
            "7:1: expected a header declaration, found `bogus`",
            "8:9: unclosed `(` opened at line 8",
            "9:14: expected end of line after the declaration `.entry @main`, found `.entry`",
        ]
    );
}

#[test]
fn malformed_header_declarations_do_not_hide_undefined_labels() {
    let text = "sst v2\n.section main\n.header\n.func @main(), locals 0\n.fnc @x(), locals 0\n\
                .entry @main\n.text\n@main: t.br @missing\n";
    assert_eq!(
        check(text),
        [
            "5:1: unknown directive `.fnc`",
            "8:13: undefined symbol `@missing`",
        ]
    );
}

#[test]
fn text_syntax_errors_are_all_reported() {
    let text = program(
        &["main"],
        "@main:\n    t.push x\n    t.jump 3\n    @done\n    t.call 0,\n    @: t.halt\n    \
         t.push 99999999999999999999\n    t.halt t.halt\n    t.push\n    1\n    t.br @done\n",
    );
    assert_eq!(
        check(&text),
        [
            "9:12: unexpected `x` in operands of `t.push`",
            "10:5: expected an instruction, found `t.jump`",
            "11:5: expected `:` after symbol name `@done`",
            "12:14: expected more operands for `t.call`, found end of line",
            "13:5: expected a symbol name after `@`",
            "14:12: invalid operand `99999999999999999999` for `t.push`",
            "15:12: expected end of line after the instruction `t.halt`, found `t.halt`",
            "16:11: expected more operands for `t.push`, found end of line",
            "17:5: expected an instruction, found `1`",
        ]
    );
}

#[test]
fn symbols_bindings_and_lowering_are_checked_together() {
    let text = program(
        &["main", "helper"],
        "@main:\n    t.call 0, @helper\n    t.br @done\n    t.print \"hi\"\n    t.print \"bye\"\n\
         @main:\n    t.halt\n",
    );
    let report = check_program::<Machine>(&source(&text));
    assert_eq!(
        summary(&report),
        [
            "6:7: function `@helper` (functions[1]) has no `@helper:` symbol definition",
            "10:15: function `@helper` has no `@helper:` symbol definition",
            "11:10: undefined symbol `@done`",
            "14:1: duplicate symbol `@main`",
        ]
    );
    let duplicate = report.diagnostics.last().unwrap();
    assert_eq!(duplicate.secondary.len(), 1);
    assert_eq!(
        source(&text)
            .location(duplicate.secondary[0].span.start)
            .to_string(),
        "9:1"
    );
}

#[test]
fn header_directive_errors_are_all_reported() {
    let text = r#"sst v2
.section main
.header
.file "f.sst"
.func @main(x: u64) -> u64, locals 0
.func @main(), locals 0
.entry @nowhere
.entry @main
.const str 4
.const fn @nope
.const u64 1
.source_symbol 1 "a"
.source_symbol 1 "b"
.text
@main: t.br @missing
"#;
    let report = check_program::<Machine>(&source(text));
    assert_eq!(
        summary(&report),
        [
            "5:29: `@main` reserves 0 local slots but has 1 parameters",
            "6:7: function `@main` is declared more than once",
            "7:8: `.entry @nowhere` names no `.func` directive",
            "8:1: the entry function is selected more than once",
            "9:8: string constants name their text: write `.const str \"...\"`",
            "10:11: `.const fn @nope` names no `.func` directive",
            "13:16: duplicate source symbol index 1",
            // `.text` is still checked against the directives that were fine.
            "15:13: undefined symbol `@missing`",
        ]
    );
    let source = source(text);
    let secondary: Vec<_> = report
        .iter()
        .flat_map(|diagnostic| &diagnostic.secondary)
        .map(|label| source.location(label.span.start).to_string())
        .collect();
    assert_eq!(secondary, ["5:7", "7:1", "12:16"]);
}

#[test]
fn an_invalid_local_count_preserves_the_declared_function_for_lowering() {
    let text = "sst v2\n.section main\n.header\n.func @main(x: u64), locals 0\n\
                .entry @main\n.const fn @main\n.text\n@main: t.call 0, @main\n  t.br @missing\n";
    assert_eq!(
        check(text),
        [
            "4:22: `@main` reserves 0 local slots but has 1 parameters",
            "9:8: undefined symbol `@missing`",
        ]
    );
}

#[test]
fn sections_without_functions_are_valid() {
    assert_eq!(
        check("sst v2\n.section main\n.text\n@start: t.halt\n"),
        Vec::<String>::new()
    );
}

#[test]
fn global_context_and_required_sections() {
    let text = format!(
        ".global\n    unexpected\n{}",
        program(&["main"], "@main: t.halt\n")
    )
    .replacen("sst v2\n", "", 1);
    let text = format!("sst v2\n{text}");
    assert_eq!(
        check(&text),
        ["2:1: invalid global context: NoContext accepts only an empty global section"]
    );

    let broken = "sst v2\n.section main\n.text\n    t.bogus\n";
    let report = ProgramChecker::<Machine>::new()
        .require_sections(["controller"])
        .check(&source(broken));
    // `main` is not selected, so only the missing section is reported.
    assert_eq!(summary(&report), ["1:1: missing section `controller`"]);
}

#[test]
fn every_section_is_checked() {
    let section = |name: &str, text: &str| {
        format!(
            ".section {name}\n.header\n{}.text\n{text}",
            header(&["main"])
        )
    };
    let text = format!(
        "sst v2\n{}{}",
        section("first", "@main: t.push x\n"),
        section("second", "@main: t.br @nowhere\n")
    );
    assert_eq!(
        check(&text),
        [
            "8:15: unexpected `x` in operands of `t.push`",
            "15:13: undefined symbol `@nowhere`",
        ]
    );
}

#[test]
fn lowering_can_be_disabled_or_customized() {
    let text = program(&["main"], "@main:\n    t.br @nowhere\n");
    let source = source(&text);
    let report = ProgramChecker::<Machine>::new()
        .without_lowering()
        .check(&source);
    assert!(report.is_ok(), "{:?}", summary(&report));

    let report = ProgramChecker::<Machine>::new()
        .with_lowering(|metadata: &ProgramMetadata, symbols: &[_]| {
            MetadataLoweringContext::new(metadata, symbols)
        })
        .check(&source);
    assert_eq!(summary(&report), ["9:10: undefined symbol `@nowhere`"]);
}

/// An application header: the standard directives plus a description.
#[allow(dead_code)] // The description is only parsed.
#[derive(Debug, vihaco_parser_derive::Parse)]
#[syntax_class(metadata)]
enum Declaration {
    #[pattern = "$0"]
    Standard(ProgramDirective),
    #[pattern = "experiment.description $0"]
    Description(String),
}

impl StandardDirective for Declaration {
    type Value = vihaco_abi::Value;
    type Type = vihaco_abi::Type;

    fn as_standard(&self) -> Option<&ProgramDirective> {
        match self {
            Self::Standard(directive) => Some(directive),
            Self::Description(_) => None,
        }
    }
}

#[test]
fn custom_header_declarations() {
    let text = format!(
        "sst v2\n.section main\n.header\nexperiment.description \"x\"\n{}.text\n@main: t.halt\n",
        header(&["main"])
    );
    let report = ProgramChecker::<Machine>::new()
        .with_header::<Declarations<Declaration>>()
        .check(&source(&text));
    assert!(report.is_ok(), "{:?}", summary(&report));
    let shared = text.replace("\"x\"\n", "\"x\" experiment.description \"y\"\n");
    let report = ProgramChecker::<Machine>::new()
        .with_header::<Declarations<Declaration>>()
        .check(&source(&shared));
    assert_eq!(
        summary(&report),
        [
            "4:28: expected end of line after the declaration `experiment.description \"x\"`, \
             found `experiment.description`"
        ]
    );
    // The standard header does not know the application directive.
    assert_eq!(
        check(&text),
        ["4:1: expected a header declaration, found `experiment.description`"]
    );
}

#[test]
fn rendered_output_shows_source_lines_and_carets() {
    let text = program(&["main"], "@main:\n    t.push x\n@main: t.halt\n");
    let source = source(&text);
    let report = check_program::<Machine>(&source);
    assert_eq!(
        report.render(&source),
        "\
error: unexpected `x` in operands of `t.push`
  --> test.sst:9:12
  |
9 |     t.push x
  |            ^ unexpected operand

error: duplicate symbol `@main`
   --> test.sst:10:1
   |
 8 | @main:
   | ------ first defined here
 9 |     t.push x
10 | @main: t.halt
   | ^^^^^^ redefined here
   |
   = symbol names must be unique within a section

error: `test.sst` has 2 errors
"
    );
    assert_eq!(
        report.diagnostics[0].to_string(),
        "9:12: error: unexpected `x` in operands of `t.push`"
    );
}
