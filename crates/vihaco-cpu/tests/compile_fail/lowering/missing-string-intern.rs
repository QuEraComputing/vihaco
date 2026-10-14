use vihaco::{Lower, Syntax};
use vihaco_cpu::{cpu, operand::StringOperand};

fn main() {
    let source = cpu::ConstString::<Syntax>(StringOperand::Resolved(0));
    let _ = source.lower(&mut ());
}
