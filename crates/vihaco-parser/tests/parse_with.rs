// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::error::Simple;
use chumsky::extra;
use chumsky::{Parser, prelude::*};
use vihaco_parser::Parse;
use vihaco_parser_core::Parse as ParseTrait;

#[derive(Debug, PartialEq)]
struct Custom(i64);

fn custom_parser<'src>() -> impl Parser<'src, &'src str, Custom, extra::Err<Simple<'src, char>>> {
    text::int(10).map(|s: &str| Custom(s.parse::<i64>().unwrap() * 2))
}

#[derive(Parse, Debug, PartialEq)]
enum A {
    Foo(#[parse_with = "custom_parser"] Custom),
}

mod parsers {
    use super::Custom;
    use chumsky::error::Simple;
    use chumsky::extra;
    use chumsky::prelude::*;
    pub fn custom_via_module<'src>()
    -> impl Parser<'src, &'src str, Custom, extra::Err<Simple<'src, char>>> {
        text::int(10).map(|s: &str| Custom(s.parse::<i64>().unwrap() * 3))
    }
}

#[derive(Parse, Debug, PartialEq)]
enum B {
    Bar(#[parse_with = "parsers::custom_via_module"] Custom),
}

#[test]
fn parse_with_field() {
    // custom_parser doubles the value, so "21" → Custom(42)
    assert_eq!(
        A::parser().parse("foo(21)").into_result().unwrap(),
        A::Foo(Custom(42))
    );
}
#[test]
fn parse_with_module_path() {
    // custom_via_module triples the value, so "14" → Custom(42)
    assert_eq!(
        B::parser().parse("bar(14)").into_result().unwrap(),
        B::Bar(Custom(42))
    );
}
