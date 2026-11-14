// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::*;
use crate::module::NoInfo;

fn directive(text: &str) -> ProgramDirective {
    ProgramDirective::parser()
        .then_ignore(end())
        .parse(text)
        .into_result()
        .unwrap_or_else(|errors| panic!("{text:?}: {errors:?}"))
}

fn header(lines: &[&str]) -> ProgramHeader {
    lines.iter().map(|line| directive(line)).collect()
}

#[test]
fn directives_parse_and_print_canonically() {
    for (source, canonical) in [
        (".file \"a.sst\"", ".file \"a.sst\""),
        (
            ".func @main() -> u64, locals 0",
            ".func @main() -> u64, locals 0",
        ),
        (
            ".func @f(x:u64 , y: i64)->(u64,bool) ,locals 2",
            ".func @f(x: u64, y: i64) -> (u64, bool), locals 2",
        ),
        (
            ".func @g(\n  x: u64, ; first\n) , locals 1",
            ".func @g(x: u64), locals 1",
        ),
        (".func @h() -> (), locals 0", ".func @h(), locals 0"),
        (".entry @main", ".entry @main"),
        (".const u64 42", ".const u64 42"),
        (".const str \"a ; b\"", ".const str \"a ; b\""),
        (".const fn @helper", ".const fn @helper"),
        (".const str 0", ".const str 0"),
        (".source_symbol 12 \"user\"", ".source_symbol 12 \"user\""),
    ] {
        let parsed = directive(source);
        let printed = parsed.to_source().unwrap();
        assert_eq!(printed, canonical, "{source:?}");
        assert_eq!(directive(&printed), parsed);
    }
    for bad in [
        ".func main() -> u64, locals 0",
        ".func @main() -> u64",
        ".func @main(x) -> u64, locals 1",
        ".entry main",
        ".files \"a\"",
        ".const",
        ".source_symbol \"x\" 1",
    ] {
        assert!(
            ProgramDirective::<Value, Type>::parser()
                .then_ignore(end())
                .parse(bad)
                .has_errors(),
            "{bad:?}"
        );
    }
}

#[test]
fn tables_are_derived_in_a_documented_order() {
    let metadata = header(&[
        ".func @first(x: u64) -> u64, locals 1",
        ".file \"b.sst\"",
        ".func @second(first: u64, y: bool), locals 3",
        ".file \"a.sst\"",
        ".func @third(), locals 0",
        ".entry @second",
        ".const u64 7",
        ".const str \"hello\"",
        ".const str \"b.sst\"",
        ".const fn @third",
        ".source_symbol 12 \"user\"",
    ])
    .finish(NoInfo)
    .unwrap();
    // Names, then parameters, then files (the first function has no `.file`),
    // then string constants; each once.
    assert_eq!(
        metadata.strings,
        [
            "first", "second", "third", "x", "y", "", "b.sst", "a.sst", "hello"
        ]
    );
    let functions: Vec<_> = metadata
        .functions
        .iter()
        .map(|f| {
            (
                f.name,
                f.signature
                    .params
                    .iter()
                    .map(|p| p.name)
                    .collect::<Vec<_>>(),
                f.local_count,
                f.file,
            )
        })
        .collect();
    assert_eq!(
        functions,
        [(0, vec![3], 1, 5), (1, vec![0, 4], 3, 6), (2, vec![], 0, 7)]
    );
    assert_eq!(metadata.main_function, Some(1));
    assert_eq!(
        metadata.constants,
        [
            Value::U64(7),
            Value::String(8),
            Value::String(6),
            Value::FunctionRef(2)
        ]
    );
    assert_eq!(metadata.source_symbols[0].name, "user");
}

#[test]
fn every_problem_is_reported_with_its_directive() {
    let errors = header(&[
        ".func @main(x: u64), locals 0",
        ".func @main(), locals 0",
        ".entry @nowhere",
        ".entry @main",
        ".const fn @nowhere",
        ".const str 0",
        ".const fn 0",
        ".source_symbol 1 \"a\"",
        ".source_symbol 1 \"b\"",
    ])
    .assemble(NoInfo)
    .unwrap_err();
    let summary: Vec<_> = errors
        .iter()
        .map(|error| (error.directive, error.focus, error.message.as_str()))
        .collect();
    assert_eq!(
        summary,
        [
            (
                0,
                Focus::Locals,
                "`@main` reserves 0 local slots but has 1 parameters"
            ),
            (
                1,
                Focus::Symbol,
                "function `@main` is declared more than once"
            ),
            (
                2,
                Focus::Symbol,
                "`.entry @nowhere` names no `.func` directive"
            ),
            (
                3,
                Focus::Directive,
                "the entry function is selected more than once"
            ),
            (
                4,
                Focus::Symbol,
                "`.const fn @nowhere` names no `.func` directive"
            ),
            (
                5,
                Focus::Argument,
                "string constants name their text: write `.const str \"...\"`"
            ),
            (
                6,
                Focus::Argument,
                "function constants name their function: write `.const fn @name`"
            ),
            (8, Focus::Argument, "duplicate source symbol index 1"),
        ]
    );
    assert_eq!(errors[1].related.as_ref().map(|r| r.0), Some(0));
    let error = header(&[".entry @x"]).finish(NoInfo).unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid header: `.entry @x` names no `.func` directive"
    );
}

#[test]
fn an_empty_header_has_empty_tables() {
    let metadata = ProgramHeader::<Value, Type>::new().finish(NoInfo).unwrap();
    assert!(metadata.strings.is_empty() && metadata.functions.is_empty());
    assert_eq!(metadata.main_function, None);
}
