use vihaco_parser::Parse;
#[derive(Parse)]
enum Bad {
    Plain(f64),
    #[delegate]
    #[token = "x"]
    A(i64),  // delegate + token — error
}
fn main() {}
