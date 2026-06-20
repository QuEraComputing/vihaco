// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser;
use vihaco_parser::Parse;
use vihaco_parser_core::Parse as ParseTrait;

#[derive(Parse, Debug, PartialEq)]
enum A {
    #[delimiters(open = "[", close = "]", separator = ";")]
    Bracket(i64, f64),

    #[delimiters(open = "", close = "", separator = ",")]
    Bare(f64, f64),

    Plain(f64),
}

#[test]
fn bracket_delimiters() {
    assert_eq!(
        A::parser().parse("bracket[42; 1.5]").into_result().unwrap(),
        A::Bracket(42, 1.5)
    );
}
#[test]
fn empty_delimiters() {
    assert_eq!(
        A::parser().parse("bare3.0, 4.0").into_result().unwrap(),
        A::Bare(3.0, 4.0)
    );
}
#[test]
fn default_delimiters() {
    assert_eq!(
        A::parser().parse("plain(9.9)").into_result().unwrap(),
        A::Plain(9.9)
    );
}
