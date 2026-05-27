use vihaco_parser::Parse;
#[derive(Parse)]
enum Bad {
    Plain(f64),
    #[delegate]
    Multi(i64, f64),  // delegate on multi-field — error
}
fn main() {}
