// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

pub mod impls;

pub use impls::ident;

use chumsky::error::Simple;
use chumsky::extra;

/// A parser whose input is `&'src str` (char stream) and whose error type is `Simple<char>`.
///
/// The lifetime `'src` is the input lifetime. Output type `Self` is owned and does not borrow
/// from the input.
pub trait Parse<'src>: Sized {
    fn parser() -> impl chumsky::Parser<'src, &'src str, Self, extra::Err<Simple<'src, char>>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chumsky::Parser;

    fn parses<'src, T: Parse<'src>>(input: &'src str) -> T {
        T::parser().parse(input).into_result().unwrap()
    }

    #[test]
    fn i64_basic() {
        assert_eq!(parses::<i64>("42"), 42);
    }
    #[test]
    fn i32_basic() {
        assert_eq!(parses::<i32>("7"), 7);
    }
    #[test]
    fn u64_basic() {
        assert_eq!(parses::<u64>("100"), 100);
    }
    #[test]
    fn u32_basic() {
        assert_eq!(parses::<u32>("0"), 0);
    }
    #[test]
    fn usize_basic() {
        assert_eq!(parses::<usize>("9"), 9);
    }
    #[test]
    fn f64_int() {
        assert_eq!(parses::<f64>("3"), 3.0);
    }
    #[test]
    #[allow(clippy::approx_constant)]
    fn f64_float() {
        assert_eq!(parses::<f64>("3.14"), 3.14);
    }
    #[test]
    fn f32_float() {
        assert!((parses::<f32>("1.5") - 1.5f32).abs() < 1e-6);
    }
    #[test]
    fn i64_negative() {
        assert_eq!(parses::<i64>("-42"), -42);
    }
    #[test]
    fn i32_negative() {
        assert_eq!(parses::<i32>("-7"), -7);
    }
    #[test]
    fn f64_negative() {
        assert_eq!(parses::<f64>("-0.5"), -0.5);
    }
    #[test]
    fn f64_negative_scientific() {
        assert_eq!(parses::<f64>("-1.0e-3"), -1.0e-3);
    }
    #[test]
    fn u64_rejects_negative() {
        assert!(u64::parser().parse("-1").into_result().is_err());
    }
    #[test]
    fn bool_true() {
        assert!(parses::<bool>("true"));
    }
    #[test]
    fn bool_false() {
        assert!(!parses::<bool>("false"));
    }
    #[test]
    fn string_word() {
        assert_eq!(parses::<String>("hello"), "hello");
    }

    #[test]
    fn string_stops_at_ws() {
        // Without a trailing end(), Parser::parse() requires consuming all input — so a
        // String parser given "hello world" fails because " world" is left unconsumed.
        // Use lazy() / nested combinators for composition; that's not this test's job.
        let result = String::parser().parse("hello world").into_result();
        assert!(result.is_err());
    }

    #[test]
    fn ident_operand_with_colons() {
        assert_eq!(
            ident().parse("AOD0:T1:A").into_result().unwrap(),
            "AOD0:T1:A"
        );
    }

    #[test]
    fn ident_stops_at_comma() {
        let result = ident()
            .then_ignore(chumsky::primitive::just(','))
            .parse("foo,")
            .into_result();
        assert_eq!(result.unwrap(), "foo");
    }

    #[test]
    fn ident_allows_dots() {
        assert_eq!(ident().parse("a.b.c").into_result().unwrap(), "a.b.c");
    }

    #[test]
    fn ident_digi_target() {
        assert_eq!(ident().parse("DIGI:0").into_result().unwrap(), "DIGI:0");
    }

    #[test]
    fn ident_rejects_empty() {
        assert!(ident().parse("").into_result().is_err());
    }

    #[test]
    fn ident_rejects_leading_ws() {
        assert!(ident().parse("  hello").into_result().is_err());
    }

    #[test]
    fn ident_stops_at_open_paren() {
        let result = ident()
            .then_ignore(chumsky::primitive::just('('))
            .parse("foo(")
            .into_result();
        assert_eq!(result.unwrap(), "foo");
    }

    #[test]
    fn ident_stops_at_brace() {
        let result = ident()
            .then_ignore(chumsky::primitive::just('{'))
            .parse("device{")
            .into_result();
        assert_eq!(result.unwrap(), "device");
    }
}
