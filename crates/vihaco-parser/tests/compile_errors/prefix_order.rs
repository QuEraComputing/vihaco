use vihaco_parser::Parse;
#[derive(Parse)]
enum Bad {
    Foo(f64),     // token "foo"
    Foobar(f64),  // token "foobar" — "foo" is prefix of "foobar", "foo" declared first — error
}
fn main() {}
