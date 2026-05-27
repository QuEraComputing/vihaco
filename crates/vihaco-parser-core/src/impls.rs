use crate::Parse;
use chumsky::error::Simple;
use chumsky::extra;
use chumsky::prelude::*;

type E<'src> = extra::Err<Simple<'src, char>>;

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

impl<'src> Parse<'src> for String {
    fn parser() -> impl Parser<'src, &'src str, Self, E<'src>> {
        any()
            .filter(|c: &char| !c.is_whitespace())
            .repeated()
            .at_least(1)
            .collect::<String>()
    }
}

pub fn ident<'src>() -> impl Parser<'src, &'src str, String, E<'src>> + Clone {
    any()
        .filter(|c: &char| {
            !c.is_whitespace() && !matches!(*c, ',' | ';' | '(' | ')' | '{' | '}' | '[' | ']')
        })
        .repeated()
        .at_least(1)
        .collect::<String>()
}
