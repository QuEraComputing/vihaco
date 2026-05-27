#![allow(clippy::approx_constant)]

use chumsky::Parser;
use vihaco_parser::Parse;
use vihaco_parser_core::Parse as ParseTrait;

#[derive(Parse, Debug, PartialEq)]
enum A {
    Foo(f64),
    Bar(i64, f64),
    Baz,
}

#[test]
fn unit_variant() {
    assert_eq!(A::parser().parse("baz").into_result().unwrap(), A::Baz);
}
#[test]
fn single_field() {
    assert_eq!(
        A::parser().parse("foo(3.14)").into_result().unwrap(),
        A::Foo(3.14)
    );
}
#[test]
fn two_fields() {
    assert_eq!(
        A::parser().parse("bar(42, 1.5)").into_result().unwrap(),
        A::Bar(42, 1.5)
    );
}
#[test]
fn two_fields_spaces() {
    assert_eq!(
        A::parser().parse("bar(42 , 1.5 )").into_result().unwrap(),
        A::Bar(42, 1.5)
    );
}
#[test]
fn space_before_paren() {
    assert_eq!(
        A::parser().parse("foo (3.14)").into_result().unwrap(),
        A::Foo(3.14)
    );
}
#[test]
fn unknown_token() {
    assert!(A::parser().parse("unknown").has_errors());
}
