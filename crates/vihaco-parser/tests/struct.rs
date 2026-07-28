// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser;
use vihaco_parser::Parse;
use vihaco_parser_core::Parse as ParseTrait;

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
#[pattern = "$left $right"]
struct Named {
    left: i64,
    right: bool,
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(instruction, head = "test")]
#[pattern = "'pair $0 $1"]
struct Tuple(i64, bool);

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(type)]
#[pattern = "`unit`"]
struct Unit;

#[test]
fn named_struct() {
    assert_eq!(
        Named::parser().parse("42 true").into_result().unwrap(),
        Named {
            left: 42,
            right: true,
        }
    );
}

#[test]
fn tuple_struct() {
    assert_eq!(
        Tuple::parser()
            .parse("test::pair 42 false")
            .into_result()
            .unwrap(),
        Tuple(42, false)
    );
}

#[test]
fn unit_struct() {
    assert_eq!(Unit::parser().parse("unit").into_result().unwrap(), Unit);
}
