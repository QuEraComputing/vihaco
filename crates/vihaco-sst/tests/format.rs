// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Formatter tests: pinned outputs for messy inputs, and the properties every
//! formatted file must have (idempotence, unchanged meaning, every user
//! comment kept, `//!` comments regenerated).

// Shared with the checker tests, which use the helpers this file does not.
#[allow(dead_code)]
mod common;

use common::{Inst, Machine, source, summary};
use vihaco_module::module::{ProgramMetadata, Symbol};
use vihaco_sst::check::{parse_declarations, parse_instructions, scan};
use vihaco_sst::{Diagnostics, FormatOptions, Formatter, ProgramChecker, format_program};

fn format_with(text: &str, options: FormatOptions) -> String {
    let source = source(text);
    match format_program::<Machine>(&source, options) {
        Ok(formatted) => formatted,
        Err(report) => panic!("{}", report.render(&source)),
    }
}

fn format(text: &str) -> String {
    format_with(text, FormatOptions::default())
}

type Parsed = Vec<(String, Vec<ProgramMetadata>, Vec<Inst>, Vec<Symbol>)>;

/// Every section's parsed header, code, and symbols.
fn parsed(text: &str) -> Parsed {
    let source = source(text);
    let mut out = Diagnostics::new(&source);
    let framing = scan(&source, &mut out);
    let sections = framing
        .sections
        .iter()
        .map(|section| {
            let header = parse_declarations::<ProgramMetadata>(section.header(), &mut out)
                .unwrap()
                .into_iter()
                .map(|spanned| spanned.value)
                .collect();
            let text = parse_instructions::<Inst>(section.text(), &mut out);
            (
                section.name().to_owned(),
                header,
                text.code.into_iter().map(|item| item.value).collect(),
                text.symbols.into_iter().map(|item| item.value).collect(),
            )
        })
        .collect();
    assert_eq!(out.error_count(), 0);
    sections
}

/// The user part of every `//` comment, outside string literals.
fn user_comments(text: &str) -> Vec<String> {
    let mut comments = Vec::new();
    for line in text.lines() {
        let mut in_string = false;
        let mut escaped = false;
        for (index, c) in line.char_indices() {
            if in_string {
                match (escaped, c) {
                    (true, _) => escaped = false,
                    (false, '\\') => escaped = true,
                    (false, '"') => in_string = false,
                    _ => {}
                }
            } else if c == '"' {
                in_string = true;
            } else if line[index..].starts_with("//") {
                let comment = line[index..].trim_end();
                if !comment.starts_with("//!") {
                    let user = comment[2..]
                        .find("//!")
                        .map_or(comment, |end| &comment[..2 + end]);
                    comments.push(user.trim_end().to_owned());
                }
                break;
            }
        }
    }
    comments.sort();
    comments
}

/// Check every formatter property for `text`, with and without generated
/// comments, and return the default formatting.
fn assert_properties(text: &str) -> String {
    let formatted = format(text);
    for (options, output) in [
        (FormatOptions::default(), formatted.clone()),
        (
            FormatOptions::without_generated_comments(),
            format_with(text, FormatOptions::without_generated_comments()),
        ),
    ] {
        assert_eq!(format_with(&output, options.clone()), output, "idempotence");
        assert_eq!(parsed(&output), parsed(text), "meaning");
        assert_eq!(user_comments(&output), user_comments(text), "comments");
        assert!(output.ends_with('\n') && !output.ends_with("\n\n"));
        assert!(!output.contains('\r'));
        assert!(output.lines().all(|line| line.trim_end() == line));
        if !options.generated_comments {
            assert!(!output.contains("//!"), "{output}");
        }
    }
    formatted
}

/// A metadata header for `main` and `helper` in `f.sst`, with string `hi`.
const HEADER: &str = r#"
metadata { strings: ["main", "helper", "f.sst", "hi"],
  functions: [{ name: 0, signature: { params: [], ret: [] }, local_count: 0, file: 2 },
              { name: 1, signature: { params: [], ret: [] }, local_count: 0, file: 2 }],
  constants: [], main_function: 0, source_symbols: [], extra: {} };
"#;

const FORMATTED_HEADER: &str = r#"    metadata {
        functions: [
            {
                name: 0,   //! "main"
                signature: { params: [], ret: [] },
                local_count: 0,
                file: 2,   //! "f.sst"
            },
            {
                name: 1,   //! "helper"
                signature: { params: [], ret: [] },
                local_count: 0,
                file: 2,   //! "f.sst"
            },
        ],
        constants: [],
        strings: ["main", "helper", "f.sst", "hi"],
        main_function: 0,  //! @main
        source_symbols: [],
        extra: {},
    };
"#;

fn program(text: &str) -> String {
    format!("sst v2\n.section main\n.header{HEADER}.text\n{text}")
}

#[test]
fn messy_text_is_one_item_per_line() {
    let text = program(
        "      @main:   t.call 0,   @helper t.print \"hi\"\n\
         \t\tt.br @done @done:t.halt\n@helper: t.push 21    t.halt",
    );
    let expected = format!(
        "sst v2\n\n.section main\n.header\n{FORMATTED_HEADER}.text\n\
@main:                 //! fn main(), locals 0, functions[0]
    t.call 0, @helper  //! @helper = 0x0004 (functions[1])
    t.print \"hi\"       //! \"hi\" = strings[3]
    t.br @done         //! @done = 0x0003
@done:                 //! 0x0003
    t.halt
@helper:               //! fn helper(), locals 0, functions[1]
    t.push 21
    t.halt
"
    );
    assert_eq!(assert_properties(&text), expected);
}

#[test]
fn crlf_input_formats_like_lf_input() {
    let lf = program("@main: t.call 0, @helper // go\n@helper:\n  t.halt\n");
    let crlf = lf.replace('\n', "\r\n");
    assert_eq!(assert_properties(&crlf), assert_properties(&lf));
}

#[test]
fn comments_everywhere_stay_with_their_items() {
    let text = r#"sst v2 // version line
.section main   // section directive
.header // header directive
  // before metadata
  metadata { // after metadata brace
    // before strings
    strings: ["main", "f.sst"], // after strings
    functions: [ // after functions bracket
      { name: 0, signature: { params: [], ret: [] }, local_count: 0, file: 1 }, // after function
      // end of functions
    ],
    constants: [], main_function: 0, source_symbols: [], extra: {}
  }; // after metadata

  // end of header
.text // text directive
// before main
@main: // on main
    t.push 1 // on push


    // before halt, after blank lines
    t.halt // on halt
  // end of text
"#;
    let expected = r#"sst v2  // version line

.section main  // section directive
.header  // header directive
    // before metadata
    metadata {                       // after metadata brace
        functions: [                 // after functions bracket
            {
                name: 0,             //! "main"
                signature: { params: [], ret: [] },
                local_count: 0,
                file: 1,             //! "f.sst"
            },                       // after function
            // end of functions
        ],
        constants: [],
        // before strings
        strings: ["main", "f.sst"],  // after strings
        main_function: 0,            //! @main
        source_symbols: [],
        extra: {},
    };                               // after metadata

// end of header
.text  // text directive
// before main
@main:        // on main //! fn main(), locals 0, functions[0]
    t.push 1  // on push

    // before halt, after blank lines
    t.halt    // on halt
    // end of text
"#;
    assert_eq!(assert_properties(text), expected);
}

#[test]
fn comments_after_a_blank_line_introduce_the_next_directive() {
    let text = format!(
        "sst v2\n.section main\n.header{HEADER}.text\n@main: t.halt\n@helper: t.halt\n\n\
         // The second section.\n.section other\n.header{HEADER}.text\n// Code.\n@main: t.halt\n@helper: t.halt\n"
    );
    let formatted = assert_properties(&text);
    assert!(
        formatted.contains("    t.halt\n\n// The second section.\n.section other\n.header\n"),
        "{formatted}"
    );
    assert!(formatted.contains(".text\n// Code.\n@main:"), "{formatted}");
}

#[test]
fn stale_generated_comments_are_replaced() {
    let text = program(
        "//! a stale own-line note\n@main: //! fn wrong()\n  t.br @done // jump //! @done = 0x9999\n\
         @done: t.halt\n@helper: t.halt //! stale\n",
    );
    let formatted = assert_properties(&text);
    assert!(!formatted.contains("stale") && !formatted.contains("wrong"));
    assert!(!formatted.contains("0x9999"));
    assert!(
        formatted.contains("    t.br @done  // jump //! @done = 0x0001\n"),
        "{formatted}"
    );
}

#[test]
fn generated_comments_can_be_turned_off() {
    let text = program("@main: t.br @done //! @done = 0x0001\n@done: t.halt\n@helper: t.halt\n");
    let formatted = format_with(&text, FormatOptions::without_generated_comments());
    let expected = "\
    metadata {
        functions: [
            { name: 0, signature: { params: [], ret: [] }, local_count: 0, file: 2 },
            { name: 1, signature: { params: [], ret: [] }, local_count: 0, file: 2 },
        ],
        constants: [],
        strings: [\"main\", \"helper\", \"f.sst\", \"hi\"],
        main_function: 0,
        source_symbols: [],
        extra: {},
    };
.text
@main:
    t.br @done
@done:
    t.halt
@helper:
    t.halt
";
    assert!(formatted.ends_with(expected), "{formatted}");
}

#[test]
fn records_break_to_fit_the_width() {
    let text = program("@main: t.halt\n@helper: t.halt\n");
    let narrow = FormatOptions {
        max_width: 40,
        ..FormatOptions::without_generated_comments()
    };
    let formatted = format_with(&text, narrow.clone());
    assert!(
        formatted.contains(
            "        strings: [\n            \"main\",\n            \"helper\",\n            \
             \"f.sst\",\n            \"hi\",\n        ],\n"
        ),
        "{formatted}"
    );
    assert_eq!(format_with(&formatted, narrow), formatted);
}

#[test]
fn constants_and_parameters_are_resolved() {
    let text = r#"sst v2
.section main
.header
metadata { strings: ["main", "x", "f.sst"],
  functions: [{ name: 0, signature: { params: [{ name: 1, ty: u64 }], ret: [u64, bool] }, local_count: 2, file: 2 }],
  constants: [str 2, fn 0, u64 7], main_function: 0, source_symbols: [], extra: {} };
.text
@main: t.halt
"#;
    let formatted = assert_properties(text);
    for expected in [
        "            str 2,         //! \"f.sst\"\n",
        "            fn 0,          //! @main\n",
        "            u64 7,\n",
        "signature: { params: [{ name: 1, ty: u64 }], ret: [u64, bool] },  //! fn main(x: u64) -> (u64, bool)\n",
        "@main:  //! fn main(x: u64) -> (u64, bool), locals 2, functions[0]\n",
    ] {
        assert!(formatted.contains(expected), "{expected}\n{formatted}");
    }
}

#[test]
fn trailing_comments_align_up_to_the_cap() {
    let text = program(
        "@main: t.halt // a\n@helper: t.print \"a much longer literal that passes the comment column\" // b\n",
    )
    .replace(r#""hi""#, r#""hi", "a much longer literal that passes the comment column""#);
    let formatted = format_with(&text, FormatOptions::without_generated_comments());
    assert!(
        formatted.ends_with(
            "@main:\n    t.halt  // a\n@helper:\n    \
             t.print \"a much longer literal that passes the comment column\"  // b\n"
        ),
        "{formatted}"
    );
}

#[test]
fn files_with_errors_return_the_checker_report() {
    let text = program("@main: t.br @nowhere\n@helper: t.halt\n");
    let report = format_program::<Machine>(&source(&text), FormatOptions::default()).unwrap_err();
    assert_eq!(summary(&report), ["9:13: undefined symbol `@nowhere`"]);
}

#[test]
fn machine_specific_annotations_are_added_by_hooks() {
    let text = program("@main: t.push 1 t.halt\n@helper: t.halt\n");
    let formatted = Formatter::<Machine>::new()
        .with_annotator(
            |instruction: &Inst, context: &vihaco_sst::InstructionContext<'_>| match instruction {
                Inst::Push(value) => vec![format!("{value} at {}", context.address)],
                _ => Vec::new(),
            },
        )
        .format(&source(&text))
        .unwrap();
    assert!(
        formatted.contains("    t.push 1  //! 1 at 0\n"),
        "{formatted}"
    );
}

#[test]
fn the_global_block_is_kept_verbatim() {
    struct AnyContext;
    impl vihaco_bytecode::SSTGlobalContext for AnyContext {
        fn from_text(_: &str) -> eyre::Result<Self> {
            Ok(Self)
        }
    }
    let text = format!(
        "sst v2\n.global // context\n  device   a {{ x }}   \n\n  // kept\n\n\n.section main\n.header{HEADER}\
         .text\n@main: t.halt\n@helper: t.halt\n"
    );
    let formatted = Formatter::with_checker(
        ProgramChecker::<Machine>::new().with_global_context::<AnyContext>(),
    )
    .options(FormatOptions::without_generated_comments())
    .format(&source(&text))
    .unwrap();
    assert!(
        formatted.starts_with(
            "sst v2\n\n.global  // context\n  device   a { x }\n\n  // kept\n\n.section main\n"
        ),
        "{formatted}"
    );
}

mod unprintable {
    use std::fmt;

    use chumsky::Parser;
    use chumsky::error::Simple;
    use chumsky::extra;
    use vihaco_abi::Lower;
    use vihaco_module::loader::MetadataLoweringContext;
    use vihaco_parser::{Parse, Print};
    use vihaco_runtime::HasInstructionSet;

    /// Parses as a number but refuses to print.
    #[derive(Debug, Clone, PartialEq)]
    pub struct Secret(u64);

    impl<'src> Parse<'src> for Secret {
        fn parser() -> impl Parser<'src, &'src str, Self, extra::Err<Simple<'src, char>>> {
            u64::parser().map(Secret)
        }
    }

    impl Print for Secret {
        fn print(&self, _: &mut dyn fmt::Write) -> fmt::Result {
            Err(fmt::Error)
        }
    }

    #[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
    #[syntax_class(instruction, head = "s")]
    pub enum Inst {
        Halt,
        Keep(Secret),
    }

    impl Lower<MetadataLoweringContext> for Inst {
        type Runtime = ();

        fn lower(self, _: &mut MetadataLoweringContext) -> eyre::Result<()> {
            Ok(())
        }
    }

    pub struct Machine;

    impl HasInstructionSet for Machine {
        type Runtime = ();
        type Syntax = Inst;
    }
}

#[test]
fn values_without_a_faithful_spelling_are_reported() {
    let text = program("@main: s.halt\n@helper: s.keep 7\n").replace("t.", "s.");
    let report = format_program::<unprintable::Machine>(&source(&text), FormatOptions::default())
        .unwrap_err();
    assert_eq!(
        summary(&report),
        ["10:10: cannot format: this value has no source spelling that parses back to itself"]
    );
}

mod declarations {
    use vihaco_module::module::{NoInfo, ProgramMetadata};
    use vihaco_sst::check::MetadataDeclaration;

    /// An application header: program metadata plus a pattern declaration
    /// whose `,` must stay on one line.
    #[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
    #[syntax_class(metadata)]
    pub enum Declaration {
        #[pattern = "$0"]
        Metadata(ProgramMetadata),
        #[pattern = "range `{` $0 `,` $1 `}`"]
        Range(u64, u64),
    }

    impl MetadataDeclaration for Declaration {
        type Value = vihaco_abi::Value;
        type Type = vihaco_abi::Type;
        type Info = NoInfo;

        fn as_metadata(&self) -> Option<&ProgramMetadata> {
            match self {
                Self::Metadata(metadata) => Some(metadata),
                Self::Range(..) => None,
            }
        }
    }
}

#[test]
fn declarations_that_cannot_break_stay_on_one_line() {
    let text = program("@main: t.halt\n@helper: t.halt\n").replace(
        ".text\n",
        "// The valid range.\nrange { 1000000000000, 2000000000000 }; // inclusive\n.text\n",
    );
    let options = FormatOptions {
        max_width: 30,
        ..FormatOptions::without_generated_comments()
    };
    let checker = ProgramChecker::<Machine>::new()
        .with_header::<vihaco_sst::Declarations<declarations::Declaration>>();
    let mut formatter = Formatter::with_checker(checker).options(options);
    let formatted = formatter.format(&source(&text)).unwrap();
    assert!(
        formatted.contains(
            "    // The valid range.\n    range{1000000000000, 2000000000000};  // inclusive\n"
        ),
        "{formatted}"
    );
    assert_eq!(formatter.format(&source(&formatted)).unwrap(), formatted);
}
