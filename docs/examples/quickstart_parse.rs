use chumsky::Parser as _;
use vihaco::Instruction;
use vihaco_parser::Parse;

// The same enum can derive both `Instruction` (bytecode + runtime) and
// `Parse` (SST). The two derives are orthogonal.
#[derive(Debug, Clone, PartialEq, Instruction, vihaco_parser_derive::Parse)]
#[syntax_class(instruction, head = "counter")]
pub enum CounterInst {
    #[pattern = "'add $0"]
    Add(i64),
    Print,
}

fn main() {
    // The syntax class supplies the `counter::` namespace. Patterns bind
    // source operands directly to Rust fields.
    let inst = CounterInst::parser()
        .parse("counter::add 5")
        .into_result()
        .unwrap();
    assert_eq!(inst, CounterInst::Add(5));
}
