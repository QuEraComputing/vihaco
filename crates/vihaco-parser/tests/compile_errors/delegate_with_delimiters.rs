use vihaco_parser::Parse;
#[derive(Parse)]
enum Bad {
    Plain(f64),
    #[delegate]
    #[delimiters(open = "[", close = "]", separator = ",")]
    A(i64),  // delegate + delimiters — error
}
fn main() {}
