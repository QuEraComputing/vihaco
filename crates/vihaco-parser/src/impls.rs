// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::Parse;
use chumsky::error::Simple;
use chumsky::extra;
use chumsky::prelude::*;
use std::{borrow::Borrow, fmt};

type E<'src> = extra::Err<Simple<'src, char>>;

/// An unquoted SST identifier, without a leading `@` sigil.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ident(pub String);

/// An unquoted SST token whose interpretation is deferred to a resolver.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BareToken(pub String);

/// The decoded contents of a quoted SST string literal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotedString(pub String);

macro_rules! impl_lexical_string {
    ($ty:ty) => {
        impl $ty {
            /// Borrow the lexical text.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consume this value and return its owned text.
            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl AsRef<str> for $ty {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl Borrow<str> for $ty {
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl From<$ty> for String {
            fn from(value: $ty) -> Self {
                value.into_inner()
            }
        }
    };
}

impl_lexical_string!(Ident);
impl_lexical_string!(BareToken);
impl_lexical_string!(QuotedString);

macro_rules! impl_uint {
    ($($t:ty),+) => {
        $(impl<'src> Parse<'src> for $t {
            fn parser() -> impl Parser<'src, &'src str, Self, E<'src>> {
                text::int(10).map(|s: &str| s.parse().unwrap())
            }
        })+
    };
}
impl_uint!(u64, u32, usize);

macro_rules! impl_sint {
    ($($t:ty),+) => {
        $(impl<'src> Parse<'src> for $t {
            fn parser() -> impl Parser<'src, &'src str, Self, E<'src>> {
                just('-')
                    .or_not()
                    .then(text::int(10))
                    .to_slice()
                    .map(|s: &str| s.parse().unwrap())
            }
        })+
    };
}
impl_sint!(i64, i32);

macro_rules! impl_float {
    ($($t:ty),+) => {
        $(impl<'src> Parse<'src> for $t {
            fn parser() -> impl Parser<'src, &'src str, Self, E<'src>> {
                // Accepts: optional unary `-`, integer, optional `.fraction`,
                // optional `e[+-]?digits`. Real .sst sources include both
                // scientific-notation literals like `1.9999999999998004e-6`
                // and negative basis components like `-0.5`; rejecting either
                // would force every consumer to roll its own float parser.
                let exp = one_of("eE")
                    .then(one_of("+-").or_not())
                    .then(text::digits(10));
                just('-')
                    .or_not()
                    .then(text::int(10))
                    .then(just('.').then(text::digits(10)).or_not())
                    .then(exp.or_not())
                    .to_slice()
                    .map(|s: &str| s.parse().unwrap())
            }
        })+
    };
}
impl_float!(f64, f32);

impl<'src> Parse<'src> for bool {
    fn parser() -> impl Parser<'src, &'src str, Self, E<'src>> {
        just("true").to(true).or(just("false").to(false))
    }
}

impl<'src> Parse<'src> for Ident {
    fn parser() -> impl Parser<'src, &'src str, Self, E<'src>> {
        ident().map(Self)
    }
}

impl<'src> Parse<'src> for BareToken {
    fn parser() -> impl Parser<'src, &'src str, Self, E<'src>> {
        bare_token().map(Self)
    }
}

impl<'src> Parse<'src> for QuotedString {
    fn parser() -> impl Parser<'src, &'src str, Self, E<'src>> {
        let escape = just('\\').ignore_then(choice((
            just('"').to('"'),
            just('\\').to('\\'),
            just('n').to('\n'),
            just('t').to('\t'),
            just('r').to('\r'),
            just('0').to('\0'),
        )));
        let character = choice((
            escape,
            any().and_is(just('"').not()).and_is(just('\\').not()),
        ));

        character
            .repeated()
            .collect::<String>()
            .delimited_by(just('"'), just('"'))
            .map(Self)
    }
}

impl<'src, T> Parse<'src> for Vec<T>
where
    T: Parse<'src> + 'src,
{
    fn parser() -> impl Parser<'src, &'src str, Self, E<'src>> {
        T::parser()
            .padded()
            .separated_by(just(',').padded())
            .collect::<Vec<_>>()
            .delimited_by(just('['), just(']'))
    }
}

impl<'src, A, B> Parse<'src> for (A, B)
where
    A: Parse<'src> + 'src,
    B: Parse<'src> + 'src,
{
    fn parser() -> impl Parser<'src, &'src str, Self, E<'src>> {
        A::parser()
            .padded()
            .then_ignore(just(',').padded())
            .then(B::parser().padded())
            .delimited_by(just('('), just(')'))
    }
}

/// Parse the text of an unquoted SST identifier without wrapping it in [`Ident`].
///
/// Dots and colons are accepted for hardware and namespaced identifiers. The
/// `@` sigil, quotes, pattern metacharacters, whitespace, and structural
/// delimiters are rejected.
pub fn ident<'src>() -> impl Parser<'src, &'src str, String, E<'src>> + Clone {
    token_text(|c| c != '@')
}

/// Parse an unquoted token without wrapping it in [`BareToken`].
///
/// Unlike [`ident`], this accepts `@` because a resolver may need to interpret
/// the token later. Quotes, pattern metacharacters, whitespace, and structural
/// delimiters are rejected.
pub fn bare_token<'src>() -> impl Parser<'src, &'src str, String, E<'src>> + Clone {
    token_text(|_| true)
}

fn token_text<'src>(
    additional_filter: impl Fn(char) -> bool + Clone + 'src,
) -> impl Parser<'src, &'src str, String, E<'src>> + Clone {
    any()
        .filter(move |c: &char| {
            !c.is_whitespace()
                && !matches!(
                    *c,
                    ',' | ';' | '(' | ')' | '{' | '}' | '[' | ']' | '"' | '\'' | '`'
                )
                && additional_filter(*c)
        })
        .repeated()
        .at_least(1)
        .collect::<String>()
}
