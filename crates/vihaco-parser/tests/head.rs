// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser;
use vihaco_parser::Parse;
use vihaco_parser_core::Parse as ParseTrait;

#[derive(Parse, Debug, PartialEq)]
#[head]
enum A {
    Foo(f64),
    Baz,
}

#[derive(Parse, Debug, PartialEq)]
#[head = "Ns::"]
enum B {
    Bar(f64),
}

#[derive(Parse, Debug, PartialEq)]
#[head]
enum C {
    #[token = "my_foo"]
    Foo(f64),
}

#[test]
fn auto_head_single_field() {
    assert_eq!(
        A::parser().parse("A::Foo(1.0)").into_result().unwrap(),
        A::Foo(1.0)
    );
}
#[test]
fn auto_head_unit() {
    assert_eq!(A::parser().parse("A::Baz").into_result().unwrap(), A::Baz);
}
#[test]
fn auto_head_rejects_bare() {
    assert!(A::parser().parse("Foo(1.0)").has_errors());
}
#[test]
fn custom_head() {
    assert_eq!(
        B::parser().parse("Ns::Bar(2.0)").into_result().unwrap(),
        B::Bar(2.0)
    );
}
#[test]
fn head_with_token_override() {
    assert_eq!(
        C::parser().parse("C::my_foo(3.0)").into_result().unwrap(),
        C::Foo(3.0)
    );
}
