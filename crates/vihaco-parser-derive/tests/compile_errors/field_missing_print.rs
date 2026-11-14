use chumsky::Parser;
use vihaco_parser_derive::Parse;

#[derive(Clone)]
struct Opaque;

// A hand-written parser without the matching printer.
impl<'src> vihaco_parser::Parse<'src> for Opaque {
    fn parser(
    ) -> impl Parser<'src, &'src str, Self, chumsky::extra::Err<chumsky::error::Simple<'src, char>>>
    {
        chumsky::primitive::just("opaque").to(Opaque)
    }
}

#[derive(Parse)]
#[syntax_class(value)]
struct Wrapper(Opaque);

fn main() {}
