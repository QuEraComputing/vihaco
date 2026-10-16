use vihaco::{Lower, Syntax};
use vihaco_cpu::{cpu, operand::Target};

fn main() {
    // Even resolved operands require the instruction's complete capability bound.
    let source = cpu::Branch::<Syntax>(Target::Resolved(10));
    let _ = source.lower(&mut ());
}
