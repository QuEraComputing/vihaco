use vihaco_runtime::{Step, composite, field};

#[composite]
struct Child {}

#[composite]
struct Machine {
    #[device]
    child: Child,
}

struct Unsupported;

impl Step<field!(Machine::child), Unsupported> for Machine {
    fn step(&mut self, _: Unsupported) {}
}

fn main() {}
