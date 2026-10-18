use vihaco_abi::{Lower, Syntax};
use vihaco_cpu::{cpu, operand::Target};

fn main() {
    let source = cpu::Call::<Syntax>(0, Target::Resolved(10));
    let _ = source.lower(&mut ());
}
