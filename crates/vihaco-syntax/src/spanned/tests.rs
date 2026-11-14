// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::*;

#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(instruction, head = "stub")]
enum Stub {
    Halt,
    Push(u32),
    Pair(u32, u32),
}

#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, head = "device")]
enum Declaration {
    #[pattern = "gain $0"]
    Gain(u64),
    #[pattern = "name $0"]
    Name(String),
}

/// `(line:column, message)` for every error, plus the text each span covers.
fn errors(text: &str, errors: &[SyntaxError]) -> Vec<(String, String, String)> {
    errors
        .iter()
        .map(|error| {
            let (line, column) = line_column(text, error.span.start);
            (
                format!("{line}:{column}"),
                text[error.span.clone()].to_owned(),
                error.message.clone(),
            )
        })
        .collect()
}

#[test]
fn text_items_keep_their_spans() {
    let text = "@main:\n    stub.push 1  // one\n@done: stub.halt\n";
    let parsed = parse_text::<Stub>(text);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let code: Vec<_> = parsed
        .code
        .iter()
        .map(|item| &text[item.span.clone()])
        .collect();
    assert_eq!(code, ["stub.push 1", "stub.halt"]);
    let symbols: Vec<_> = parsed
        .symbols
        .iter()
        .map(|item| (&text[item.span.clone()], item.value.address))
        .collect();
    assert_eq!(symbols, [("@main:", 0), ("@done:", 1)]);
}

#[test]
fn text_errors_are_readable_and_all_reported() {
    let text = "@main:\n  foo bar\n  stub.push x\n  @done\n  stub.pair 1,\n  @: stub.halt\n  stub.push 99999999999\n  stub.halt\n";
    let parsed = parse_text::<Stub>(text);
    assert_eq!(
        errors(text, &parsed.errors),
        [
            (
                "2:3".into(),
                "foo".into(),
                "expected an instruction, found `foo`".into()
            ),
            (
                "3:13".into(),
                "x".into(),
                "unexpected `x` in operands of `stub.push`".into()
            ),
            (
                "4:3".into(),
                "@done".into(),
                "expected `:` after symbol name `@done`".into()
            ),
            (
                "5:15".into(),
                "".into(),
                "expected more operands for `stub.pair`, found end of line".into()
            ),
            (
                "6:3".into(),
                "@".into(),
                "expected a symbol name after `@`".into()
            ),
            (
                "7:13".into(),
                "99999999999".into(),
                "invalid operand `99999999999` for `stub.push`".into()
            ),
        ]
    );
    // Recovered items keep the addresses of the instructions around them.
    assert_eq!(parsed.code.len(), 1);
    let symbols: Vec<_> = parsed
        .symbols
        .iter()
        .map(|item| (item.value.name.as_str(), item.value.address))
        .collect();
    assert_eq!(symbols, [("main", 0), ("done", 0)]);
}

#[test]
fn declarations_keep_spans_and_report_every_error() {
    let text = "device gain 2;\ndevice gain x;\nbogus;\ndevice name \"a\"\ndevice gain 3;\n";
    let parsed = parse_declarations::<Declaration>(text);
    assert_eq!(
        errors(text, &parsed.errors),
        [
            (
                "2:13".into(),
                "x".into(),
                "unexpected `x` in `device` declaration".into()
            ),
            (
                "3:1".into(),
                "bogus".into(),
                "expected a header declaration, found `bogus`".into()
            ),
            (
                "4:16".into(),
                "".into(),
                "expected `;` after `device` declaration".into()
            ),
        ]
    );
    let spans: Vec<_> = parsed
        .declarations
        .iter()
        .map(|item| &text[item.span.clone()])
        .collect();
    assert_eq!(
        spans,
        ["device gain 2", "device name \"a\"", "device gain 3"]
    );
}

#[test]
fn unterminated_trailing_declaration_is_reported_at_the_end() {
    let text = "device gain 2;\ndevice name\n";
    let parsed = parse_declarations::<Declaration>(text);
    assert_eq!(
        errors(text, &parsed.errors),
        [(
            "2:12".into(),
            "".into(),
            "unexpected end of header in `device` declaration".into()
        )]
    );
}
