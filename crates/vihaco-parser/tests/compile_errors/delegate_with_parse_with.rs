use vihaco_parser::Parse;
fn my_parser<'src>() -> impl chumsky::Parser<'src, &'src str, i64, chumsky::extra::Err<chumsky::error::Simple<'src, char>>> {
    use chumsky::Parser as _;
    chumsky::text::int(10).map(|s: &str| s.parse().unwrap())
}
#[derive(Parse)]
enum Bad {
    Plain(f64),
    #[delegate]
    A(#[parse_with = "my_parser"] i64),  // delegate + parse_with on field — error
}
fn main() {}
