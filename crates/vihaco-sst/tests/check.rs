// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Fixture-style checker tests: each malformed file asserts the exact set of
//! diagnostics, as `line:column: message`.

mod common;

use common::{Machine, function, header, source, summary};
use vihaco_module::loader::MetadataLoweringContext;
use vihaco_module::module::ProgramMetadata;
use vihaco_sst::check::MetadataDeclaration;
use vihaco_sst::{Declarations, ProgramChecker, check_framing, check_program};

fn check(text: &str) -> Vec<String> {
    summary(&check_program::<Machine>(&source(text)))
}

/// A one-section file whose header declares `main` (and, if given, more
/// functions) with strings `strings`. The `.text` block starts on line 13.
fn program(strings: &str, functions: &[String], text: &str) -> String {
    format!(
        "sst v2\n.section main\n.header\n{}.text\n{text}",
        header(strings, &functions.join(", "), "0")
    )
}

#[test]
fn a_well_formed_program_checks_clean() {
    let text = program(
        r#""main", "helper", "f.sst", "hi""#,
        &[function(0, 2), function(1, 2)],
        "@main:\n    t.call 0, @helper\n    t.print \"hi\"\n    t.br @done\n@done: t.halt\n\
         @helper:\n    t.push 21 // the answer, halved\n    t.halt\n",
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
    let text = "sst v2\n.section main\n.header\n    metadata {\n        strings: [\"main\"],\n        \
                functins: [],\n    };\n    bogus;\n.text\n@main:\n    t.halt\n";
    assert_eq!(
        check(text),
        [
            "6:9: unexpected `functins` in `metadata` declaration",
            "8:5: expected a header declaration, found `bogus`",
        ]
    );
}

#[test]
fn text_syntax_errors_are_all_reported() {
    let text = program(
        r#""main", "f.sst""#,
        &[function(0, 1)],
        "@main:\n    t.push x\n    t.jump 3\n    @done\n    t.call 0,\n    @: t.halt\n    \
         t.push 99999999999999999999\n    t.halt t.halt\n    t.br @done\n",
    );
    assert_eq!(
        check(&text),
        [
            "14:12: unexpected `x` in operands of `t.push`",
            "15:5: expected an instruction, found `t.jump`",
            "16:5: expected `:` after symbol name `@done`",
            "17:14: expected more operands for `t.call`, found end of line",
            "18:5: expected a symbol name after `@`",
            "19:12: invalid operand `99999999999999999999` for `t.push`",
        ]
    );
}

#[test]
fn symbols_bindings_and_lowering_are_checked_together() {
    let text = program(
        r#""main", "helper", "f.sst", "hi""#,
        &[function(0, 2), function(1, 2)],
        "@main:\n    t.call 0, @helper\n    t.br @done\n    t.print \"hi\"\n    t.print \"bye\"\n\
         @main:\n    t.halt\n",
    );
    let report = check_program::<Machine>(&source(&text));
    assert_eq!(
        summary(&report),
        [
            "6:103: function `@helper` (metadata.functions[1]) has no `@helper:` symbol definition",
            "14:15: function `@helper` has no `@helper:` symbol definition",
            "15:10: undefined symbol `@done`",
            "17:13: string literal `bye` is missing from metadata.strings",
            "18:1: duplicate symbol `@main`",
        ]
    );
    let duplicate = report.diagnostics.last().unwrap();
    assert_eq!(duplicate.secondary.len(), 1);
    assert_eq!(
        source(&text)
            .location(duplicate.secondary[0].span.start)
            .to_string(),
        "13:1"
    );
}

#[test]
fn metadata_table_errors_are_all_reported() {
    let text = r#"sst v2
.section main
.header
    metadata {
        strings: ["main", "f.sst"],
        functions: [
            { name: 0, signature: { params: [{ name: 7, ty: u64 }], ret: [] }, local_count: 0, file: 5 },
            { name: 9, signature: { params: [], ret: [] }, local_count: 0, file: 1 },
            { name: 0, signature: { params: [], ret: [] }, local_count: 0, file: 1 },
        ],
        constants: [string 4, fn 8, u64 1],
        main_function: 3,
        source_symbols: [{ index: 1, name: "a" }, { index: 1, name: "b" }],
        extra: {},
    };
.text
@main: t.halt
"#;
    assert_eq!(
        check(text),
        [
            "7:54: metadata.functions[0].signature.params[0].name: string index 7 is out of \
             range (metadata.strings has 2 entries)",
            "7:93: metadata.functions[0].local_count: 0 local slots cannot hold 1 parameters",
            "7:102: metadata.functions[0].file: string index 5 is out of range \
             (metadata.strings has 2 entries)",
            "8:21: metadata.functions[1].name: string index 9 is out of range \
             (metadata.strings has 2 entries)",
            "9:21: function `@main` is declared by both metadata.functions[0] and \
             metadata.functions[2]",
            "11:21: metadata.constants[0]: string index 4 is out of range (metadata.strings \
             has 2 entries)",
            "11:31: metadata.constants[1]: function index 8 is out of range \
             (metadata.functions has 3 entries)",
            "12:24: metadata.main_function: function index 3 is out of range \
             (metadata.functions has 3 entries)",
            "13:60: metadata.source_symbols[1]: duplicate index 1",
        ]
    );
}

#[test]
fn duplicate_function_is_reported_at_the_redeclaration() {
    let text = program(
        r#""main", "f.sst""#,
        &[function(0, 1), function(0, 1)],
        "@main: t.halt\n",
    );
    let report = check_program::<Machine>(&source(&text));
    let [diagnostic] = report.diagnostics.as_slice() else {
        panic!("{:?}", summary(&report));
    };
    assert_eq!(diagnostic.location.to_string(), "6:103");
    assert_eq!(
        source(&text)
            .location(diagnostic.secondary[0].span.start)
            .to_string(),
        "6:29"
    );
}

#[test]
fn missing_and_duplicate_metadata_declarations() {
    assert_eq!(
        check("sst v2\n.section main\n.text\n@main: t.halt\n"),
        ["2:1: section `main` has no program metadata declaration"]
    );
    let metadata = header(r#""main", "f.sst""#, &function(0, 1), "0");
    let text =
        format!("sst v2\n.section main\n.header\n{metadata}{metadata}.text\n@main: t.halt\n");
    assert_eq!(
        check(&text),
        ["12:5: section `main` declares program metadata more than once"]
    );
}

#[test]
fn global_context_and_required_sections() {
    let text = format!(
        ".global\n    unexpected\n{}",
        program(r#""main", "f.sst""#, &[function(0, 1)], "@main: t.halt\n")
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
            header(r#""main", "f.sst""#, &function(0, 1), "0")
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
            "13:15: unexpected `x` in operands of `t.push`",
            "25:13: undefined symbol `@nowhere`",
        ]
    );
}

#[test]
fn lowering_can_be_disabled_or_customized() {
    let text = program(
        r#""main", "f.sst""#,
        &[function(0, 1)],
        "@main:\n    t.br @nowhere\n",
    );
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
    assert_eq!(summary(&report), ["14:10: undefined symbol `@nowhere`"]);
}

/// An application header: program metadata plus a description.
#[allow(dead_code)] // The description is only parsed.
#[derive(Debug, vihaco_parser_derive::Parse)]
#[syntax_class(metadata)]
enum Declaration {
    #[pattern = "$0"]
    Metadata(ProgramMetadata),
    #[pattern = "experiment.description $0"]
    Description(String),
}

impl MetadataDeclaration for Declaration {
    type Value = vihaco_abi::Value;
    type Type = vihaco_abi::Type;
    type Info = vihaco_module::module::NoInfo;

    fn as_metadata(&self) -> Option<&ProgramMetadata> {
        match self {
            Self::Metadata(metadata) => Some(metadata),
            Self::Description(_) => None,
        }
    }
}

#[test]
fn custom_header_declarations() {
    let metadata = header(r#""main", "f.sst""#, &function(0, 1), "0");
    let text = format!(
        "sst v2\n.section main\n.header\n    experiment.description \"x\"\n{metadata}.text\n\
         @main: t.halt\n"
    );
    let report = ProgramChecker::<Machine>::new()
        .with_header::<Declarations<Declaration>>()
        .check(&source(&text));
    assert_eq!(
        summary(&report),
        ["4:31: expected `;` after `experiment.description` declaration"]
    );
    let fixed = text.replace("\"x\"\n", "\"x\";\n");
    let report = ProgramChecker::<Machine>::new()
        .with_header::<Declarations<Declaration>>()
        .check(&source(&fixed));
    assert!(report.is_ok(), "{:?}", summary(&report));
}

#[test]
fn rendered_output_shows_source_lines_and_carets() {
    let text = program(
        r#""main", "f.sst""#,
        &[function(0, 1)],
        "@main:\n    t.push x\n@main: t.halt\n",
    );
    let source = source(&text);
    let report = check_program::<Machine>(&source);
    assert_eq!(
        report.render(&source),
        "\
error: unexpected `x` in operands of `t.push`
   --> test.sst:14:12
   |
14 |     t.push x
   |            ^ unexpected operand

error: duplicate symbol `@main`
   --> test.sst:15:1
   |
13 | @main:
   | ------ first defined here
14 |     t.push x
15 | @main: t.halt
   | ^^^^^^ redefined here
   |
   = symbol names must be unique within a section

error: `test.sst` has 2 errors
"
    );
    assert_eq!(
        report.diagnostics[0].to_string(),
        "14:12: error: unexpected `x` in operands of `t.push`"
    );
}
