// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Canonical source printing: the inverse of [`Parse`](crate::Parse).
//!
//! Every type with a `Parse` implementation in this crate also implements
//! [`Print`], and `#[derive(Parse)]` derives both from the same pattern, so
//! `parse(print(x)) == x` holds for every printable value.
//!
//! Output is canonical and minimal: one space between tokens, `, ` between
//! list items, no newlines, and no leading or trailing whitespace. Layout
//! (indentation, line breaks) is left to callers, so printed fragments compose.
//!
//! Printing fails with [`fmt::Error`] when a value has no source spelling that
//! parses back to the same value, for example a non-finite `f64`, an empty or
//! malformed [`Ident`], or `Some(x)` whose spelling of `x` would be read as
//! `null`.

use crate::impls::{BareToken, Ident, QuotedString};
use std::fmt;

/// Write a value in the source spelling accepted by its [`Parse`](crate::Parse)
/// implementation.
///
/// Implementations must round-trip: parsing the printed text yields a value
/// equal to `self`. When no such spelling exists, return [`fmt::Error`] rather
/// than writing text that would parse differently.
///
/// [`fmt::Formatter`] implements [`fmt::Write`], so a `Display` impl can
/// forward to `print`.
pub trait Print {
    /// Write the canonical source spelling of `self` to `out`.
    fn print(&self, out: &mut dyn fmt::Write) -> fmt::Result;

    /// Return the canonical source spelling of `self` as a string.
    fn to_source(&self) -> eyre::Result<String> {
        let mut source = String::new();
        self.print(&mut source)
            .map_err(|_| eyre::eyre!("value has no source spelling that parses back to itself"))?;
        Ok(source)
    }
}

impl<T: Print + ?Sized> Print for &T {
    fn print(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        T::print(self, out)
    }
}

impl<T: Print + ?Sized> Print for Box<T> {
    fn print(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        T::print(self, out)
    }
}

macro_rules! impl_display_print {
    ($($t:ty),+) => {
        $(impl Print for $t {
            fn print(&self, out: &mut dyn fmt::Write) -> fmt::Result {
                write!(out, "{self}")
            }
        })+
    };
}
impl_display_print!(u64, u32, usize, i64, i32, bool);

macro_rules! impl_float_print {
    ($($t:ty),+) => {
        $(impl Print for $t {
            /// Print the shortest decimal that parses back to the same bits.
            ///
            /// Non-finite values have no source spelling and fail.
            fn print(&self, out: &mut dyn fmt::Write) -> fmt::Result {
                if !self.is_finite() {
                    return Err(fmt::Error);
                }
                // `Debug` is the shortest round-tripping form and always
                // includes a fraction or exponent (`1.0`, `1e-7`), both of
                // which the float parser accepts.
                write!(out, "{self:?}")
            }
        })+
    };
}
impl_float_print!(f64, f32);

/// Whether the unquoted token parsers accept `c` inside a token.
fn is_token_char(c: char) -> bool {
    !c.is_whitespace()
        && !matches!(
            c,
            ',' | ';' | '(' | ')' | '{' | '}' | '[' | ']' | '"' | '\'' | '`'
        )
}

/// Write an unquoted token after checking that the token parser reads it back.
///
/// Tokens never contain `;`, which starts a line comment.
fn print_token(text: &str, allow_at: bool, out: &mut dyn fmt::Write) -> fmt::Result {
    if text.is_empty()
        || !text
            .chars()
            .all(|c| is_token_char(c) && (allow_at || c != '@'))
    {
        return Err(fmt::Error);
    }
    out.write_str(text)
}

impl Print for Ident {
    fn print(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        print_token(self.as_str(), false, out)
    }
}

impl Print for BareToken {
    fn print(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        print_token(self.as_str(), true, out)
    }
}

/// Write `text` as a double-quoted string literal using the escapes accepted
/// by the [`QuotedString`] parser.
pub fn print_quoted(text: &str, out: &mut dyn fmt::Write) -> fmt::Result {
    out.write_char('"')?;
    for c in text.chars() {
        match c {
            '"' => out.write_str("\\\"")?,
            '\\' => out.write_str("\\\\")?,
            '\n' => out.write_str("\\n")?,
            '\t' => out.write_str("\\t")?,
            '\r' => out.write_str("\\r")?,
            '\0' => out.write_str("\\0")?,
            c => out.write_char(c)?,
        }
    }
    out.write_char('"')
}

impl Print for QuotedString {
    fn print(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        print_quoted(self.as_str(), out)
    }
}

impl Print for String {
    fn print(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        print_quoted(self, out)
    }
}

impl Print for str {
    fn print(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        print_quoted(self, out)
    }
}

impl<T: Print> Print for Vec<T> {
    fn print(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        out.write_char('[')?;
        for (index, item) in self.iter().enumerate() {
            if index > 0 {
                out.write_str(", ")?;
            }
            item.print(out)?;
        }
        out.write_char(']')
    }
}

impl<A: Print, B: Print> Print for (A, B) {
    fn print(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        out.write_char('(')?;
        self.0.print(out)?;
        out.write_str(", ")?;
        self.1.print(out)?;
        out.write_char(')')
    }
}

impl<T: Print> Print for Option<T> {
    /// `None` prints as `null`. `Some(x)` prints as `x`, unless that spelling
    /// would itself be read as the `null` keyword.
    fn print(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        match self {
            None => out.write_str("null"),
            Some(value) => {
                let mut text = String::new();
                value.print(&mut text)?;
                // `null` is matched as a whole identifier; conservatively
                // treat any non-ASCII continuation as possibly ending it.
                if let Some(rest) = text.strip_prefix("null") {
                    let continues_ident = rest
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
                    if !continues_ident {
                        return Err(fmt::Error);
                    }
                }
                out.write_str(&text)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Parse;
    use chumsky::Parser;
    use std::fmt::Debug;

    fn round_trip<T>(value: T) -> String
    where
        T: Print + Debug + PartialEq + for<'src> Parse<'src>,
    {
        let source = value.to_source().unwrap();
        let parsed = T::parser().parse(source.as_str()).into_result();
        assert_eq!(parsed.as_ref(), Ok(&value), "reparsing {source:?}");
        source
    }

    #[test]
    fn integers_round_trip_at_their_bounds() {
        for value in [0, 1, i64::MIN, i64::MAX, -42] {
            round_trip(value);
        }
        for value in [i32::MIN, i32::MAX, -7] {
            round_trip(value);
        }
        for value in [0, u64::MAX] {
            round_trip(value);
        }
        for value in [0, u32::MAX] {
            round_trip(value);
        }
        round_trip(usize::MAX);
        assert_eq!(round_trip(-42_i64), "-42");
    }

    #[test]
    fn bools_round_trip() {
        assert_eq!(round_trip(true), "true");
        assert_eq!(round_trip(false), "false");
    }

    #[test]
    fn floats_round_trip_exactly() {
        let values = [
            0.0,
            -0.0,
            1.0,
            -1.5,
            0.1,
            1e-7,
            -1.9999999999998004e-6,
            1e16,
            1e300,
            f64::MIN_POSITIVE,
            5e-324,
            f64::MAX,
            f64::MIN,
        ];
        for value in values {
            let source = value.to_source().unwrap();
            let parsed = f64::parser().parse(source.as_str()).into_result().unwrap();
            assert_eq!(parsed.to_bits(), value.to_bits(), "{source}");
        }
        for value in [0.0_f32, -0.0, 1.5, -3.25e-12, f32::MAX, f32::MIN_POSITIVE] {
            let source = value.to_source().unwrap();
            let parsed = f32::parser().parse(source.as_str()).into_result().unwrap();
            assert_eq!(parsed.to_bits(), value.to_bits(), "{source}");
        }
        assert_eq!(1.0_f64.to_source().unwrap(), "1.0");
        assert_eq!((-0.5_f64).to_source().unwrap(), "-0.5");
    }

    #[test]
    fn non_finite_floats_have_no_spelling() {
        assert!(f64::NAN.to_source().is_err());
        assert!(f64::INFINITY.to_source().is_err());
        assert!(f32::NEG_INFINITY.to_source().is_err());
    }

    #[test]
    fn lexical_tokens_round_trip_and_reject_unparseable_text() {
        assert_eq!(round_trip(Ident("AOD0:T1:A".into())), "AOD0:T1:A");
        assert_eq!(round_trip(Ident("a.b.c".into())), "a.b.c");
        assert_eq!(round_trip(BareToken("@target".into())), "@target");
        for bad in ["", "two words", "a,b", "@x", "q\"", ";c", "x)"] {
            assert!(Ident(bad.into()).to_source().is_err(), "{bad:?}");
        }
        assert!(BareToken("".into()).to_source().is_err());
        assert!(BareToken("a b".into()).to_source().is_err());
    }

    #[test]
    fn strings_escape_exactly_what_the_parser_unescapes() {
        let text = "quote\" back\\slash\nnew\ttab\rret\0nul é 🦀 '";
        assert_eq!(
            round_trip(QuotedString(text.into())),
            "\"quote\\\" back\\\\slash\\nnew\\ttab\\rret\\0nul é 🦀 '\""
        );
        round_trip(String::from(text));
        assert_eq!(round_trip(String::new()), "\"\"");
    }

    #[test]
    fn collections_use_canonical_separators() {
        assert_eq!(round_trip(vec![1.0, 2.5]), "[1.0, 2.5]");
        assert_eq!(round_trip(Vec::<u32>::new()), "[]");
        assert_eq!(round_trip((1_i64, 2.5_f64)), "(1, 2.5)");
        assert_eq!(
            round_trip(vec![(1.0, 2.0), (3.0, -4.0)]),
            "[(1.0, 2.0), (3.0, -4.0)]"
        );
        assert_eq!(
            round_trip(vec![vec![Ident("a".into())], vec![]]),
            "[[a], []]"
        );
    }

    #[test]
    fn options_print_null_and_reject_ambiguous_values() {
        assert_eq!(round_trip(None::<u32>), "null");
        assert_eq!(round_trip(Some(3_u32)), "3");
        assert_eq!(round_trip(Some(String::from("null"))), "\"null\"");
        assert_eq!(round_trip(Some(Ident("nullable".into()))), "nullable");
        assert!(Some(Ident("null".into())).to_source().is_err());
        assert!(Some(Ident("null.x".into())).to_source().is_err());
        assert!(Some(None::<u32>).to_source().is_err());
    }
}
