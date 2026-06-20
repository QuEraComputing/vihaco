// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser;
use vihaco_parser::Parse;
use vihaco_parser_core::Parse as ParseTrait;

#[derive(Parse, Debug, PartialEq)]
enum A {
    Triple(i64, f64, bool),
}

#[test]
fn three_field_variant() {
    let result = A::parser()
        .parse("triple(1, 2.5, true)")
        .into_result()
        .unwrap();
    assert_eq!(result, A::Triple(1, 2.5, true));
}

#[test]
fn three_field_with_spaces() {
    let result = A::parser()
        .parse("triple(1 , 2.5 , true )")
        .into_result()
        .unwrap();
    assert_eq!(result, A::Triple(1, 2.5, true));
}
