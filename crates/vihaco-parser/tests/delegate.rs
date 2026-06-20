// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser;
use vihaco_parser::Parse;
use vihaco_parser_core::Parse as ParseTrait;

#[derive(Parse, Debug, PartialEq)]
#[head]
enum Inner {
    Foo(f64),
    Bar,
}

#[derive(Parse, Debug, PartialEq)]
enum Outer {
    #[delegate]
    Inner(Inner),
}

#[derive(Parse, Debug, PartialEq)]
#[head]
enum Inner2 {
    One(f64),
}

#[derive(Parse, Debug, PartialEq)]
enum Multi {
    #[delegate]
    A(Inner),
    #[delegate]
    B(Inner2),
}

#[test]
fn delegate_single_field() {
    assert_eq!(
        Outer::parser()
            .parse("Inner::Foo(3.0)")
            .into_result()
            .unwrap(),
        Outer::Inner(Inner::Foo(3.0))
    );
}
#[test]
fn delegate_unit() {
    assert_eq!(
        Outer::parser().parse("Inner::Bar").into_result().unwrap(),
        Outer::Inner(Inner::Bar)
    );
}
#[test]
fn delegate_no_wrapper_token() {
    assert!(Outer::parser().parse("inner(Inner::Foo(3.0))").has_errors());
}
#[test]
fn multiple_delegates_first() {
    assert_eq!(
        Multi::parser()
            .parse("Inner::Foo(1.0)")
            .into_result()
            .unwrap(),
        Multi::A(Inner::Foo(1.0))
    );
}
#[test]
fn multiple_delegates_second() {
    assert_eq!(
        Multi::parser()
            .parse("Inner2::One(2.0)")
            .into_result()
            .unwrap(),
        Multi::B(Inner2::One(2.0))
    );
}
