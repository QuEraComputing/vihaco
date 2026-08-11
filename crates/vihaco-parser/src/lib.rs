// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

pub mod impls;

pub use impls::{bare_token, ident, BareToken, Ident, QuotedString};

pub use chumsky::Parser;
pub use chumsky::{error::Simple, extra};

/// Marker for enums whose pattern-derived parser represents instruction
/// syntax.
///
/// `#[derive(vihaco_parser_derive::Parse)]` implements this trait for enums annotated
/// with `#[syntax_class(instruction, ...)]`.
pub trait SurfaceInstruction {}

/// The optional source-syntax product owned by a runtime component.
///
/// Components implement this contract when they provide local instruction,
/// value, and source-type syntax. The parser implementations are required for
/// every input lifetime so a composite can compose the products without
/// knowing how they are mounted.
pub trait InstructionSet {
    /// The component's surface instruction syntax.
    type Instruction: SurfaceInstruction + for<'src> Parse<'src>;
    /// The component's operand/value syntax.
    type Value: for<'src> Parse<'src>;
    /// The component's source-type syntax.
    type Type: for<'src> Parse<'src>;
}

/// Prefix a component parser with a public composite namespace.
///
/// This helper keeps generated syntax code independent of the parser crate's
/// implementation details while allowing a mounted component to retain its
/// local grammar.
pub fn namespaced_parser<'src, T>(
    namespace: &'static str,
) -> impl Parser<'src, &'src str, T, extra::Err<Simple<'src, char>>>
where
    T: Parse<'src> + 'src,
{
    chumsky::text::ascii::ident()
        .to_slice()
        .filter(move |name: &&str| *name == namespace)
        .then_ignore(chumsky::primitive::just("::"))
        .ignore_then(T::parser())
}

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
    fn ident_word() {
        assert_eq!(parses::<Ident>("hello"), Ident("hello".to_owned()));
    }

    #[test]
    fn ident_stops_at_whitespace() {
        // Without a trailing end(), Parser::parse() requires consuming all input — so a
        // token parser given "hello world" fails because " world" is left unconsumed.
        // Use lazy() / nested combinators for composition; that's not this test's job.
        let result = Ident::parser().parse("hello world").into_result();
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
    fn ident_rejects_symbol_sigil_and_quote_characters() {
        assert!(Ident::parser().parse("@target").has_errors());
        assert!(Ident::parser().parse("\"target\"").has_errors());
        assert!(Ident::parser().parse("'target").has_errors());
        assert!(Ident::parser().parse("`target`").has_errors());
    }

    #[test]
    fn bare_token_accepts_symbol_sigil_but_rejects_quotes() {
        assert_eq!(
            parses::<BareToken>("@target"),
            BareToken("@target".to_owned())
        );
        assert!(BareToken::parser().parse("\"target\"").has_errors());
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

    #[test]
    fn quoted_string_supports_spaces_and_escapes() {
        assert_eq!(
            parses::<QuotedString>("\"hello\\nworld\""),
            QuotedString("hello\nworld".to_owned())
        );
    }

    #[test]
    fn lexical_newtypes_expose_owned_and_borrowed_text() {
        let ident = Ident("target".to_owned());
        assert_eq!(ident.as_str(), "target");
        assert_eq!(ident.to_string(), "target");
        assert_eq!(String::from(ident), "target");
    }

    #[test]
    fn vec_uses_square_brackets_and_commas() {
        assert_eq!(parses::<Vec<f64>>("[1.0, 2.5]"), vec![1.0, 2.5]);
        assert!(parses::<Vec<f64>>("[]").is_empty());
    }

    #[test]
    fn tuple_uses_parentheses_and_a_comma() {
        assert_eq!(parses::<(i64, f64)>("(1, 2.5)"), (1, 2.5));
    }

    #[test]
    fn vec_supports_nested_tuple_items() {
        assert_eq!(
            parses::<Vec<(f64, f64)>>("[(1.0, 2.0), (3.0, 4.0)]"),
            vec![(1.0, 2.0), (3.0, 4.0)]
        );
    }
}
