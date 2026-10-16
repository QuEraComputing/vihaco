use vihaco_parser_derive::Parse;

#[derive(Parse)]
#[syntax_class(instruction)]
#[pattern = "'call $0:Missing"]
struct Call(u32);

fn main() {}
