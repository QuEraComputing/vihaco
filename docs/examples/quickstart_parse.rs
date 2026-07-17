use chumsky::Parser as _;
use vihaco::Instruction;
use vihaco_parser_core::Parse;

// The same enum can derive both `Instruction` (bytecode + runtime) and
// `Parse` (SST). The two derives are orthogonal.
#[derive(Debug, Clone, PartialEq, Instruction, vihaco_parser::Parse)]
pub enum CounterInst {
    Add(i64),
    Print,
}

fn main() {
    // The default token is the lowercase variant name; tuple fields are
    // wrapped in `( )` by default. (Bare forms like `add 5` are opt-in via
    // `#[delimiters(open = "", close = "", separator = "")]`.)
    let inst = CounterInst::parser()
        .parse("add(5)")
        .into_result()
        .unwrap();
    assert_eq!(inst, CounterInst::Add(5));
}
