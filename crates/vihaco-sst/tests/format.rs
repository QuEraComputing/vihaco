// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Formatter tests: pinned outputs for messy inputs, and the properties every
//! formatted file must have (idempotence, unchanged meaning, every user
//! comment kept, `;;` comments regenerated).

// Shared with the checker tests, which use the helpers this file does not.
#[allow(dead_code)]
mod common;

use common::{Inst, Machine, source, summary};
use vihaco_module::module::{ProgramDirective, Symbol};
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

type Parsed = Vec<(String, Vec<ProgramDirective>, Vec<Inst>, Vec<Symbol>)>;

/// Every section's parsed header, code, and symbols.
fn parsed(text: &str) -> Parsed {
    let source = source(text);
    let mut out = Diagnostics::new(&source);
    let framing = scan(&source, &mut out);
    let sections = framing
        .sections
        .iter()
        .map(|section| {
            let header = parse_declarations::<ProgramDirective>(section.header(), &mut out)
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

/// Whether an own-line comment is generated.
fn generated(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with(";;") && !line.starts_with(";;;")
}

/// Every user `;` comment, outside string literals: all comments except the
/// generated own-line `;;` ones.
fn user_comments(text: &str) -> Vec<String> {
    let mut comments = Vec::new();
    for line in text.lines() {
        if generated(line) {
            continue;
        }
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
            } else if c == ';' {
                comments.push(line[index..].trim_end().to_owned());
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
            assert!(!output.lines().any(generated), "{output}");
        }
    }
    formatted
}

/// A messy header for `main` and `helper` in `f.sst`.
const HEADER: &str = "\n  .file   \"f.sst\"\n.func @main( ) , locals 0\n   .func @helper(),locals 0\n\
                      .entry   @main\n";

fn program(text: &str) -> String {
    format!("sst v2\n.section main\n.header{HEADER}.text\n{text}")
}

#[test]
fn messy_text_is_one_item_per_line() {
    let text = program(
        "      @main:   t.call 0,   @helper\n t.print \"hi\"\n\
         \t\tt.br @done\n\n\n @done:t.halt\n@helper: t.push 21    \n\tt.halt",
    );
    let expected = "sst v2

.section main
.header
    .file \"f.sst\"
    ;; functions[0], @main = 0x0000..0x0004
    .func @main(), locals 0
    ;; functions[1], @helper = 0x0004..0x0006
    .func @helper(), locals 0
    .entry @main
.text
;; fn main(), locals 0, functions[0]
@main:
    ;; @helper = 0x0004 (functions[1])
    t.call 0, @helper
    ;; \"hi\" = strings[3]
    t.print \"hi\"
    ;; @done = 0x0003
    t.br @done

;; 0x0003
@done:
    t.halt
;; fn helper(), locals 0, functions[1]
@helper:
    t.push 21
    t.halt
";
    assert_eq!(assert_properties(&text), expected);
}

#[test]
fn crlf_input_formats_like_lf_input() {
    let lf = program("@main: t.call 0, @helper ; go\n@helper:\n  t.halt\n");
    let crlf = lf.replace('\n', "\r\n");
    assert_eq!(assert_properties(&crlf), assert_properties(&lf));
}

#[test]
fn comments_everywhere_stay_with_their_items() {
    let text = r#"sst v2 ; version line
.section main   ; section directive
.header ; header directive
  ; before file
  .file "f.sst" ; on file

  ;;; a heading is a user comment
  .func @main(), locals 0 ; on main
  .entry @main
  ; end of header
.text ; text directive
; before main
@main: ; on main
    t.push 1 ; on push


    ; before halt, after blank lines
    t.halt ; on halt
  ; end of text
"#;
    let expected = r#"sst v2  ; version line

.section main  ; section directive
.header  ; header directive
    ; before file
    .file "f.sst"            ; on file

    ;;; a heading is a user comment
    ;; functions[0], @main = 0x0000..0x0002
    .func @main(), locals 0  ; on main
    .entry @main
    ; end of header
.text  ; text directive
; before main
;; fn main(), locals 0, functions[0]
@main:        ; on main
    t.push 1  ; on push

    ; before halt, after blank lines
    t.halt    ; on halt
    ; end of text
"#;
    assert_eq!(assert_properties(text), expected);
}

#[test]
fn comments_outside_blocks_stay_above_the_next_directive() {
    let text = format!(
        "; top\nsst v2\n; after version\n\n\n; second\n.section main\n; before header\n\
         .header{HEADER}.text\n@main: t.halt\n@helper: t.halt\n"
    );
    let formatted = assert_properties(&text);
    assert!(
        formatted.starts_with(
            "; top\nsst v2\n\n; after version\n\n; second\n.section main\n; before header\n.header\n"
        ),
        "{formatted}"
    );
}

#[test]
fn comments_at_the_end_of_a_block_stay_in_it() {
    let text = format!(
        "sst v2\n.section main\n.header{HEADER}\n; End of header.\n.text\n@main: t.halt\n@helper: t.halt\n\n\
         ; The second section.\n.section other\n.header{HEADER}.text\n; Code.\n@main: t.halt\n@helper: t.halt\n"
    );
    let formatted = assert_properties(&text);
    assert!(
        formatted.contains("    .entry @main\n\n    ; End of header.\n.text\n"),
        "{formatted}"
    );
    assert!(
        formatted.contains("    t.halt\n\n    ; The second section.\n\n.section other\n.header\n"),
        "{formatted}"
    );
    assert!(
        formatted.contains(".text\n; Code.\n;; fn main()"),
        "{formatted}"
    );
}

#[test]
fn stale_generated_comments_are_replaced() {
    let text = program(
        ";; a stale own-line note\n;; fn wrong()\n@main:\n  ;; @done = 0x9999\n  t.br @done ; jump\n\
         @done: t.halt\n@helper: t.halt ; see ;; this is the user's\n",
    )
    .replace(".entry   @main\n", ";; stale header note\n.entry   @main\n");
    let formatted = assert_properties(&text);
    assert!(!formatted.contains("stale") && !formatted.contains("wrong"));
    assert!(!formatted.contains("0x9999"));
    assert!(
        formatted.contains("    ;; @done = 0x0001\n    t.br @done  ; jump\n"),
        "{formatted}"
    );
    assert!(
        formatted.contains("    t.halt      ; see ;; this is the user's\n"),
        "{formatted}"
    );
}

#[test]
fn generated_comments_can_be_turned_off() {
    let text = program("@main:\n;; @done = 0x0001\nt.br @done\n@done: t.halt\n@helper: t.halt\n");
    let formatted = format_with(&text, FormatOptions::without_generated_comments());
    let expected = "\
.header
    .file \"f.sst\"
    .func @main(), locals 0
    .func @helper(), locals 0
    .entry @main
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
fn constants_and_parameters_are_resolved() {
    let text = r#"sst v2
.section main
.header
.file "f.sst"
.func @main(x: u64) -> (u64, bool), locals 2
.entry @main
.const u64 7
.const str "f.sst"
.const fn @main
.const str "new"
.source_symbol 3 "loop"
.text
@main: t.print "hi"
"#;
    let formatted = assert_properties(text);
    for expected in [
        "    ;; functions[0], @main = 0x0000..0x0001\n    .func @main(x: u64) -> (u64, bool), locals 2\n",
        "    ;; constants[0]\n    .const u64 7\n",
        "    ;; constants[1] = strings[2]\n    .const str \"f.sst\"\n",
        "    ;; constants[2] = functions[0]\n    .const fn @main\n",
        "    ;; constants[3] = strings[3]\n    .const str \"new\"\n",
        "\n    .source_symbol 3 \"loop\"\n",
        ";; fn main(x: u64) -> (u64, bool), locals 2, functions[0]\n@main:\n",
        // Names, parameters, files, string constants, then `.text` literals.
        "    ;; \"hi\" = strings[4]\n    t.print \"hi\"\n",
    ] {
        assert!(formatted.contains(expected), "{expected}\n{formatted}");
    }
}

#[test]
fn trailing_comments_align_up_to_the_cap() {
    let text = program(
        "@main: t.halt ; a\n@helper: t.print \"a much longer literal that passes the comment column\" ; b\n",
    );
    let formatted = format_with(&text, FormatOptions::without_generated_comments());
    assert!(
        formatted.ends_with(
            "@main:\n    t.halt  ; a\n@helper:\n    \
             t.print \"a much longer literal that passes the comment column\"  ; b\n"
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
    let text = program("@main: t.push 1\nt.halt\n@helper: t.halt\n");
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
        formatted.contains("    ;; 1 at 0\n    t.push 1\n"),
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
        "sst v2\n.global ; context\n  device   a {{ x }}   \n\n  ; kept\n\n\n.section main\n.header{HEADER}\
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
            "sst v2\n\n.global  ; context\n  device   a { x }\n\n  ; kept\n\n.section main\n"
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
    let text = program("@main: s.halt\n@helper: s.keep 7\n");
    let report = format_program::<unprintable::Machine>(&source(&text), FormatOptions::default())
        .unwrap_err();
    assert_eq!(
        summary(&report),
        ["10:10: cannot format: this value has no source spelling that parses back to itself"]
    );
}

mod declarations {
    use vihaco_module::module::{ProgramDirective, StandardDirective};

    /// An application header: the standard directives plus a list that may
    /// break across lines and a pattern declaration whose `,` must stay on
    /// one line.
    #[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
    #[syntax_class(metadata)]
    pub enum Declaration {
        #[pattern = "$0"]
        Standard(ProgramDirective),
        #[pattern = "range `{` $0 `,` $1 `}`"]
        Range(u64, u64),
        #[pattern = "sites $0"]
        Sites(Vec<u64>),
    }

    impl StandardDirective for Declaration {
        type Value = vihaco_abi::Value;
        type Type = vihaco_abi::Type;

        fn as_standard(&self) -> Option<&ProgramDirective> {
            match self {
                Self::Standard(directive) => Some(directive),
                Self::Range(..) | Self::Sites(_) => None,
            }
        }
    }
}

#[test]
fn application_directives_keep_their_place_and_break_only_inside_brackets() {
    let text = program("@main: t.halt\n@helper: t.halt\n").replace(
        ".text\n",
        "; The valid range.\nrange { 1000000000000, 2000000000000 } ; inclusive\n\
         sites [1, ; first\n  2000000000, 3000000000]\n.text\n",
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
            "    .entry @main\n    ; The valid range.\n    range{1000000000000, 2000000000000}  ; inclusive\n\
             \x20   sites [\n        1,                               ; first\n        2000000000,\n        3000000000,\n    ]\n.text\n"
        ),
        "{formatted}"
    );
    assert_eq!(formatter.format(&source(&formatted)).unwrap(), formatted);
}
