use chumsky::Parser as _;
use vihaco::Instruction;
use vihaco_parser::Parse;

// This standalone source/bytecode enum is independent of component products.
// A composite owns the runtime instruction sum when it combines components.
// The two derives remain orthogonal.
#[derive(Debug, Clone, PartialEq, Instruction, vihaco_parser_derive::Parse)]
#[syntax_class(instruction)]
pub enum CounterInst {
    #[pattern = "'counter::add $0"]
    Add(i64),
    Print,
}

fn main() {
    // Patterns include the complete source token and bind operands directly
    // to Rust fields.
    let inst = CounterInst::parser()
        .parse("counter::add 5")
        .into_result()
        .unwrap();
    assert_eq!(inst, CounterInst::Add(5));
}
