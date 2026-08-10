use chumsky::Parser as _;
use vihaco::Instruction;
use vihaco_parser::Parse;

// The same enum can derive both `Instruction` (bytecode + runtime) and
// `Parse` (SST). The two derives are orthogonal.
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
