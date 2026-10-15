#[derive(Clone, Debug, PartialEq)]
struct HaltSurface;
impl<'src> vm::Parse<'src> for HaltSurface {
    fn parser() -> impl vm::chumsky::Parser<'src, &'src str, Self,
        vm::chumsky::extra::Err<vm::chumsky::error::Simple<'src, char>>> {
        use vm::chumsky::Parser as _;
        vm::chumsky::primitive::just("foo.halt").map(|_| HaltSurface)
    }
}
fn main() {
    use vm::{Parse, chumsky::Parser};
    assert_eq!(HaltSurface::parser().parse("foo.halt").into_result().unwrap(), HaltSurface);
}
