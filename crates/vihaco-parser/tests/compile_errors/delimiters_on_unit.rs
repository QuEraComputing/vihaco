use vihaco_parser::Parse;
#[derive(Parse)]
enum Bad {
    #[delimiters(open = "[", close = "]", separator = ",")]
    Unit,  // delimiters on unit variant — error
}
fn main() {}
