#[derive(vm::Parse)]
#[syntax_class(instruction, head = "cpu")]
#[pattern = "'loop"]
struct r#loop;
fn main() {}
