use vihaco_parser::Parse;
#[derive(Parse)]
enum Bad {
    #[delegate]
    A(i64),
    Plain(f64),  // token-bearing variant after delegate — error
}
fn main() {}
